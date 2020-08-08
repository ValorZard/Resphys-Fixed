use super::object::ColliderHandle;

/// Event generated by the physics engine.  
/// In case of an overlap between a solid body and sensor the solid body is guaranteed to be the first handle.
#[derive(Debug, Clone, Copy)]
pub enum PhysicsEvent<T> {
    OverlapStarted(ColliderHandle, ColliderHandle, T, T),
    OverlapEnded(ColliderHandle, ColliderHandle, T, T),
    CollisionStarted(ColliderHandle, ColliderHandle, T, T),
    CollisionEnded(ColliderHandle, ColliderHandle, T, T),
}
