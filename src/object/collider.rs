use super::super::collision::{self, ContactManifold, AABB};
use super::super::collision::{Ray, Raycast};
use super::body_set::BodyHandle;
use crate::{Vec2, FP};

/// Describes a collider in the shape of `Shape`. Attached to a body.
#[derive(Clone, Debug)]
pub struct Collider<T> {
    /// Currently the only shape is `AABB`
    pub shape: AABB,
    /// Offset from the body's position, 0 for centered
    pub offset: Vec2,
    /// Whether to treat the body as physical or not
    pub state: ColliderState,
    /// Ideally only one bit should be set
    pub category_bits: u32,
    /// Bodies only collide if both of their masks match
    pub mask_bits: u32,
    /// User supplied tag for identification
    pub user_tag: T,
    /// Body who owns the collider
    pub owner: BodyHandle,
}

impl<T> Collider<T> {
    pub fn new(
        shape: AABB,
        offset: Vec2,
        state: ColliderState,
        category_bits: u32,
        mask_bits: u32,
        user_tag: T,
        owner: BodyHandle,
    ) -> Self {
        Self {
            shape,
            offset,
            state,
            category_bits,
            mask_bits,
            user_tag,
            owner,
        }
    }
    pub fn overlaps_aabb(&self, own_position: Vec2, position: Vec2, half_exts: Vec2) -> bool {
        let own_position = own_position + self.offset;
        collision::intersection_aabb_aabb(own_position, self.shape.half_exts, position, half_exts)
    }
    pub fn ray_contact(&self, own_position: Vec2, ray: &Ray) -> Option<Raycast> {
        let own_position = own_position + self.offset;
        collision::contact_ray_aabb(ray, own_position, self.shape.half_exts)
    }
}

/// Boolean test whether two `Colliders` collided.
pub fn is_colliding<T>(
    collider1: &Collider<T>,
    position1: Vec2,
    collider2: &Collider<T>,
    position2: Vec2,
) -> bool {
    // apply offset
    let position1 = position1 + collider1.offset;
    let position2 = position2 + collider2.offset;
    collision::intersection_aabb_aabb(
        position1,
        collider1.shape.half_exts,
        position2,
        collider2.shape.half_exts,
    )
}

pub fn is_penetrating<T>(
    collider1: &Collider<T>,
    position1: Vec2,
    collider2: &Collider<T>,
    position2: Vec2,
    tolerance: FP,
) -> bool {
    let position1 = position1 + collider1.offset;
    let position2 = position2 + collider2.offset;
    collision::intersection_aabb_aabb(
        position1,
        collider1.shape.half_exts - Vec2::splat(tolerance),
        position2,
        collider2.shape.half_exts,
    )
}

/// Generates a ContactManifold if two `Colliders` collided.
pub fn collision_manifold<T>(
    collider1: &Collider<T>,
    position1: Vec2,
    collider2: &Collider<T>,
    position2: Vec2,
) -> Option<ContactManifold> {
    // apply offset
    let position1 = position1 + collider1.offset;
    let position2 = position2 + collider2.offset;
    collision::contact_aabb_aabb(
        position1,
        collider1.shape.half_exts,
        position2,
        collider2.shape.half_exts,
    )
}

/// State of the collider, determines default collision resolution and types of events sent.
#[derive(Copy, Clone, Debug)]
pub enum ColliderState {
    /// Solid body resolves collision.
    Solid,
    /// Sensor sends events about possible overlap.
    Sensor,
}
