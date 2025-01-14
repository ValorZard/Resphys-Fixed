use super::collision::{CollisionGraph, CollisionInfo, Interaction, Ray, Raycast};
use super::event::ContactEvent;
use super::object::{
    collision_manifold, is_colliding, is_penetrating, Body, BodyHandle, BodySet, BodyStatus,
    Collider, ColliderHandle, ColliderSet, ColliderState,
};
use crate::{to_fp, Vec2, FP};

/// T - User supplied type used as a tag, present in all events
pub struct PhysicsWorld<T> {
    pub collision_graph: CollisionGraph,
    pub(crate) events: Vec<ContactEvent<T>>,
    removal_events: Vec<ContactEvent<T>>,
    body_handles: Vec<BodyHandle>,
}

impl<T: Copy> Default for PhysicsWorld<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Copy> PhysicsWorld<T> {
    pub fn new() -> Self {
        Self {
            collision_graph: CollisionGraph::with_capacity(128, 16),
            events: Vec::with_capacity(16),
            removal_events: Vec::with_capacity(8),
            body_handles: Vec::with_capacity(16),
        }
    }
    /// Panics if there's no collider associated with the handle.  
    /// When collider has active collisions/overlaps the Ended event is scheduled to be sent next frame.
    pub fn remove_collider(
        &mut self,
        handle: ColliderHandle,
        bodies: &mut BodySet,
        colliders: &mut ColliderSet<T>,
    ) {
        #[cfg(debug_assertions)]
        if colliders.get(handle).is_none() {
            panic!("Trying to delete nonexistent collider {:?}", handle)
        }
        let collider = colliders.internal_remove(handle);
        let collision_graph = &mut self.collision_graph;
        let removal_events = &mut self.removal_events;

        // schedule collision/overlap ended events
        let node_index = collision_graph.get_node_index(handle);
        for node_index_other in collision_graph.src.neighbors(node_index) {
            let handle_other = *collision_graph
                .src
                .node_weight(node_index_other)
                .expect("remove_collider: other node missing");
            let collider_other = &colliders[handle_other];
            let event =
                ContactEvent::new(handle, &collider, handle_other, collider_other).into_finished();
            removal_events.push(event);
        }
        collision_graph.remove_node(handle);

        // if owner doesn't exist it's assumed both collider and body are getting removed
        if let Some(body) = bodies.get_mut(collider.owner) {
            let index = body
                .colliders
                .iter()
                .position(|owned_handle| *owned_handle == handle);
            if let Some(index) = index {
                body.colliders.swap_remove(index);
            } else {
                #[cfg(debug_assertions)]
                panic!(
                    "Body {:?} didn't know about {:?} collider",
                    collider.owner, handle
                )
            }
        }
    }
    /// Panics if there's no body associated with the handle.  
    /// All associated colliders are also removed.
    /// When any collider has active collisions/overlaps the Ended event is scheduled to be sent next frame.
    pub fn remove_body(
        &mut self,
        handle: BodyHandle,
        bodies: &mut BodySet,
        colliders: &mut ColliderSet<T>,
    ) {
        #[cfg(debug_assertions)]
        if bodies.get(handle).is_none() {
            panic!("Trying to delete nonexistent body {:?}", handle)
        }
        let body = bodies.internal_remove(handle);
        for collider_handle in body.colliders.into_iter() {
            self.remove_collider(collider_handle, bodies, colliders);
        }
    }

    /// Interactions are defined per collider.  
    /// To get only collisions or overlaps use `collisions_of` or `overlaps_of` respectively.  
    pub fn interactions_of(
        &self,
        handle: ColliderHandle,
    ) -> impl Iterator<Item = (crate::ColliderHandle, &Interaction)> {
        self.collision_graph.edges(handle)
    }
    /// Interactions are defined per collider.  
    /// To get only collisions or overlaps use `collisions_of` or `overlaps_of` respectively.  
    pub fn collisions_of(
        &self,
        handle: ColliderHandle,
    ) -> impl Iterator<Item = (crate::ColliderHandle, &CollisionInfo)> {
        self.collision_graph
            .edges(handle)
            .filter_map(|(h, interaction)| Some((h, interaction.collision()?)))
    }
    /// Interactions are defined per collider.  
    /// To get only collisions or overlaps use `collisions_of` or `overlaps_of` respectively.  
    pub fn overlaps_of(
        &self,
        handle: ColliderHandle,
    ) -> impl Iterator<Item = (crate::ColliderHandle, &Interaction)> {
        self.collision_graph
            .edges(handle)
            .filter(|(_h, interaction)| interaction.is_overlap())
    }
    /// Returns an iterator to `ColliderHandle`'s of colliders overlapping with given AABB.  
    ///  `position` is the center of the AABB
    pub fn overlap_test<'a>(
        &self,
        position: Vec2,
        half_exts: Vec2,
        collision_mask: u32,
        bodies: &'a BodySet,
        colliders: &'a ColliderSet<T>,
    ) -> impl Iterator<Item = ColliderHandle> + 'a {
        // TODO: Use broadphase
        bodies
            .iter()
            .flat_map(|(_, body)| body.colliders.iter().map(move |h| (*h, body.position)))
            .filter(move |(h, _)| (colliders[*h].category_bits & collision_mask) != 0)
            .filter_map(move |(h, body_pos)| {
                if colliders[h].overlaps_aabb(body_pos, position, half_exts) {
                    Some(h)
                } else {
                    None
                }
            })
    }
    /// Returns an iterator to `ColliderHandle`'s of colliders overlapping with given ray.  
    pub fn project_ray<'a>(
        &self,
        ray: &'a Ray,
        collision_mask: u32,
        bodies: &'a BodySet,
        colliders: &'a ColliderSet<T>,
    ) -> impl Iterator<Item = (ColliderHandle, Raycast)> + 'a {
        // TODO: Use broadphase
        bodies
            .iter()
            .flat_map(|(_, body)| body.colliders.iter().map(move |h| (*h, body.position)))
            .filter(move |(h, _)| (colliders[*h].category_bits & collision_mask) != 0)
            .filter_map(move |(h, pos)| {
                colliders[h]
                    .ray_contact(pos, ray)
                    .map(|raycast| (h, raycast))
            })
    }
    pub fn events(&self) -> &Vec<ContactEvent<T>> {
        &self.events
    }

    pub fn step(&mut self, dt: FP, bodies: &mut BodySet, colliders: &mut ColliderSet<T>) {
        self.events.clear();
        self.events.append(&mut self.removal_events);
        self.body_handles.clear();

        let collision_graph = &mut self.collision_graph;
        let events = &mut self.events;
        let body_handles = &mut self.body_handles;

        body_handles.extend(bodies.iter().map(|(h, _)| h));

        // compute the new maximum movement for every body
        for (_, body) in bodies.iter_mut() {
            if let BodyStatus::Kinematic = body.status {
                body.movement = body.velocity.mul_scalar(dt);
            }
        }

        step_x(bodies, colliders, body_handles);
        step_y(bodies, colliders, collision_graph, body_handles);

        describe_collisions(bodies, colliders, collision_graph, events);

        // for (h1, _h2, manifold) in manifolds.iter() {
        //     let body = bodies.get_mut(*h1).expect("Body missing post collision");
        //     let contact = manifold.best_contact();
        //     // body.position -= contact.normal * contact.depth;

        //     *body.velocity.x_mut() *= contact.normal.y().abs();
        //     *body.velocity.y_mut() *= contact.normal.x().abs();
        // }
    }
}

fn step_x<T>(bodies: &mut BodySet, colliders: &mut ColliderSet<T>, body_handles: &[BodyHandle]) {
    for body1_handle in body_handles {
        let body1 = bodies.get(*body1_handle).expect("Collider without a body");
        let mut move_x = body1.movement.x();

        if let BodyStatus::Static = body1.status {
            continue;
        }

        for coll1_handle in &body1.colliders {
            let collider1 = colliders
                .get(*coll1_handle)
                .expect("Body cached nonexistent collider");

            // for x step we skip sensors completely
            if let ColliderState::Sensor = collider1.state {
                continue;
            }

            // TODO: Broadphase scan just the neighbours
            for (coll2_handle, collider2) in colliders.iter() {
                // no collider colliding with itself
                if *coll1_handle == coll2_handle {
                    continue;
                }

                // for x step we skip sensors completely
                if let ColliderState::Sensor = collider2.state {
                    continue;
                }

                if !can_collide(body1, collider1, collider2) {
                    continue;
                }

                let body2 = bodies
                    .get(collider2.owner)
                    .expect("Collider without a body");

                if is_penetrating(
                    collider1,
                    body1.position + Vec2::new(move_x, to_fp(0.)),
                    collider2,
                    body2.position,
                    to_fp(0.001),
                ) {
                    if body1.velocity.x() > 0. {
                        move_x = move_x.min(
                            body2.position.x() - collider1.offset.x() + collider2.offset.x()
                                - collider2.shape.half_exts.x()
                                - collider1.shape.half_exts.x()
                                - body1.position.x(),
                        );
                    } else {
                        move_x = move_x.max(
                            body2.position.x() - collider1.offset.x()
                                + collider2.offset.x()
                                + collider2.shape.half_exts.x()
                                + collider1.shape.half_exts.x()
                                - body1.position.x(),
                        );
                    }
                }
            }
        }
        let body1 = bodies
            .get_mut(*body1_handle)
            .expect("Collider without a body");
        *body1.position.x_mut() += move_x;
    }
}

fn step_y<T>(
    bodies: &mut BodySet,
    colliders: &mut ColliderSet<T>,
    collision_graph: &mut CollisionGraph,
    body_handles: &[BodyHandle],
) {
    for body1_handle in body_handles {
        let body1 = bodies.get(*body1_handle).expect("Collider without a body");
        let mut move_y = body1.movement.y();

        if let BodyStatus::Static = body1.status {
            continue;
        }

        for coll1_handle in body1.colliders.iter() {
            let collider1 = colliders
                .get(*coll1_handle)
                .expect("Body cached nonexistent collider");

            // TODO: Broadphase scan just the neighbours
            for (coll2_handle, collider2) in colliders.iter() {
                let coll2_handle = coll2_handle;
                // no collider colliding with itself
                if *coll1_handle == coll2_handle {
                    continue;
                }

                if !can_collide(body1, collider1, collider2) {
                    continue;
                }

                let body2 = bodies
                    .get(collider2.owner)
                    .expect("Collider without a body");

                if let (ColliderState::Solid, ColliderState::Solid) =
                    (collider1.state, collider2.state)
                {
                    if is_penetrating(
                        collider1,
                        body1.position + Vec2::new(to_fp(0.), move_y),
                        collider2,
                        body2.position,
                        to_fp(0.001),
                    ) {
                        if body1.velocity.y() > 0. {
                            move_y = move_y.min(
                                body2.position.y() - collider1.offset.y() + collider2.offset.y()
                                    - collider2.shape.half_exts.y()
                                    - collider1.shape.half_exts.y()
                                    - body1.position.y(),
                            );
                        } else {
                            move_y = move_y.max(
                                body2.position.y() - collider1.offset.y()
                                    + collider2.offset.y()
                                    + collider2.shape.half_exts.y()
                                    + collider1.shape.half_exts.y()
                                    - body1.position.y(),
                            );
                        }
                    }
                }
                if is_colliding(collider1, body1.position, collider2, body2.position) {
                    collision_graph.update_edge(*coll1_handle, coll2_handle);
                }
            }
        }
        let body1 = bodies
            .get_mut(*body1_handle)
            .expect("Collider without a body");
        *body1.position.y_mut() += move_y;
    }
}

fn can_collide<T>(body1: &Body, collider1: &Collider<T>, collider2: &Collider<T>) -> bool {
    let category_mismatch = ((collider1.category_bits & collider2.mask_bits) == 0)
        || ((collider2.category_bits & collider1.mask_bits) == 0);
    // only colliders with matching masks can collide
    if category_mismatch {
        return false;
    }

    // don't collide with same body if it's disabled
    if collider1.owner == collider2.owner && !body1.self_collide {
        return false;
    }
    true
}

fn describe_collisions<T: Copy>(
    bodies: &BodySet,
    colliders: &ColliderSet<T>,
    collision_graph: &mut CollisionGraph,
    events: &mut Vec<ContactEvent<T>>,
) {
    // TODO: Don't reallocate
    let mut removed_edges = vec![];

    // collision event and contact information
    for edge_id in collision_graph.src.edge_indices() {
        let (node1_id, node2_id) = collision_graph.src.edge_endpoints(edge_id).unwrap();
        let handle1 = collision_graph.src[node1_id];
        let handle2 = collision_graph.src[node2_id];
        let collider1 = &colliders[handle1];
        let collider2 = &colliders[handle2];

        let previous_interaction = collision_graph.src.edge_weight_mut(edge_id).unwrap();

        let position1 = bodies
            .get(collider1.owner)
            .expect("Collider without a body")
            .position;
        let position2 = bodies
            .get(collider2.owner)
            .expect("Collider without a body")
            .position;

        let current_interaction = {
            use ColliderState::Solid;
            if let (Solid, Solid) = (collider1.state, collider2.state) {
                if let Some(manifold) =
                    collision_manifold(collider1, position1, collider2, position2)
                {
                    // manifolds.push((collider1.owner.0, collider2.owner.0, manifold));
                    Some(Interaction::Collision(CollisionInfo::from(
                        manifold.best_contact(),
                    )))
                } else {
                    None
                }
            } else if is_colliding(collider1, position1, collider2, position2) {
                Some(Interaction::Overlap)
            } else {
                None
            }
        };

        if current_interaction.is_some() && previous_interaction.is_none() {
            events.push(ContactEvent::new(handle1, collider1, handle2, collider2));
        }
        if current_interaction.is_none() {
            removed_edges.push((node1_id, node2_id));
            if previous_interaction.is_some() {
                events.push(
                    ContactEvent::new(handle1, collider1, handle2, collider2).into_finished(),
                );
            }
        }
        *previous_interaction = current_interaction;
    }

    removed_edges.into_iter().for_each(|(node1_id, node2_id)| {
        if let Some(edge_id) = collision_graph.src.find_edge(node1_id, node2_id) {
            if collision_graph.src.remove_edge(edge_id).is_none() {
                log::debug!("CollisionGraph error: Invalid edge removed")
            }
        } else {
            log::debug!(
                "CollisionGraph error: No edge between {:?} and {:?}",
                node1_id,
                node2_id
            );
        }
    });
}
