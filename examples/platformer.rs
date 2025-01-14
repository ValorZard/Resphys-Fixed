use macroquad::prelude::*;
use resphys::{Collider, ColliderState, AABB, FP};

// Crude character controller

extern crate log;

const FPS_INV: f32 = 1. / 60.;

type PhysicsWorld = resphys::PhysicsWorld<TagType>;
type Vec2 = resphys::Vec2;

#[macroquad::main("Controllable box")]
async fn main() {
    let mut physics = PhysicsWorld::new();
    let mut bodies = resphys::BodySet::new();
    let mut colliders = resphys::ColliderSet::new();

    let body1 = resphys::builder::BodyDesc::new()
        .with_position(Vec2::from(360., 285.))
        .self_collision(false)
        .build();
    let collider1 = resphys::builder::ColliderDesc::new(
        AABB {
            half_exts: Vec2::from(16., 32.),
        },
        TagType::Player,
    );

    let player_bhandle = bodies.insert(body1);
    let _player_chandle = colliders
        .insert(collider1.build(player_bhandle), &mut bodies, &mut physics)
        .unwrap();

    for x in (0..=768).step_by(32) {
        add_tile(
            &mut physics,
            &mut bodies,
            &mut colliders,
            Vec2::from(16. + x as f32, 16.),
        );
    }
    for y in (32..=544).step_by(32) {
        add_tile(
            &mut physics,
            &mut bodies,
            &mut colliders,
            Vec2::from(16., 16. + y as f32),
        );
    }
    for y in (32..=544).step_by(32) {
        add_tile(
            &mut physics,
            &mut bodies,
            &mut colliders,
            Vec2::from(768. + 16., 16. + y as f32),
        );
    }
    for x in (32..=768 - 32).step_by(32) {
        add_tile(
            &mut physics,
            &mut bodies,
            &mut colliders,
            Vec2::from(16. + x as f32, 544. + 16.),
        );
    }

    let mut remaining_time = 0.;
    loop {
        remaining_time += get_frame_time();
        while remaining_time >= FPS_INV {
            let player_body = &mut bodies[player_bhandle];

            player_body.velocity = player_body.velocity + Vec2::from(0., 64. * FPS_INV);

            player_body.velocity = controls(player_body.velocity);

            physics.step(FP::from_num(FPS_INV), &mut bodies, &mut colliders);
            remaining_time -= FPS_INV;
        }

        clear_background(Color::new(0., 1., 1., 1.));
        for (_, collider) in colliders.iter() {
            let body = &bodies[collider.owner];
            draw_collider(&collider, body.position);
        }

        next_frame().await
    }
}

// 32 is tile per second
fn controls(mut velocity: Vec2) -> Vec2 {
    let input: f32 = {
        if is_key_down(KeyCode::Left) {
            -1.
        } else if is_key_down(KeyCode::Right) {
            1.
        } else {
            0.
        }
    };

    velocity = velocity + Vec2::from(input * 32. * 8. * FPS_INV, 0.);

    // if movement pressed

    let damped = FP::from_num((1f32 - 0.2).powf(5. * FPS_INV));
    *velocity.x_mut() *= damped;
    // println!("vel: {}", velocity.x());

    *velocity.x_mut() = velocity
        .x()
        .max(FP::from_num(-32. * 4.))
        .min(FP::from_num(32. * 4.));

    if is_key_pressed(KeyCode::Up) {
        velocity = velocity + Vec2::from(0., -128.);
    }
    velocity
}

fn add_tile(
    physics: &mut PhysicsWorld,
    bodies: &mut resphys::BodySet,
    colliders: &mut resphys::ColliderSet<TagType>,
    position: Vec2,
) {
    let body3 = resphys::builder::BodyDesc::new()
        .with_position(position)
        .make_static()
        .build();
    let collider3 = resphys::builder::ColliderDesc::new(
        AABB {
            half_exts: Vec2::from(16., 16.),
        },
        TagType::Tile,
    );
    let body3_handle = bodies.insert(body3);
    colliders.insert(collider3.build(body3_handle), bodies, physics);
}

fn draw_collider(collider: &Collider<TagType>, position: Vec2) {
    let mut color = match collider.state {
        ColliderState::Solid => BLUE,
        ColliderState::Sensor => YELLOW,
    };
    // Quickly change color's alpha
    let fill_color = color;

    color.a = 0.3;
    // This works because there's currently only AABB shape. Half extents.
    let wh = collider.shape.half_exts;
    let x_pos = FP::to_num::<f32>(position.x() - wh.x() + collider.offset.x());
    let y_pos = FP::to_num::<f32>(position.y() - wh.y() + collider.offset.y());
    draw_rectangle(
        x_pos,
        y_pos,
        FP::to_num::<f32>(wh.x()) * 2.,
        FP::to_num::<f32>(wh.y()) * 2.,
        color,
    );
    draw_rectangle_lines(
        x_pos,
        y_pos,
        FP::to_num::<f32>(wh.x()) * 2.,
        FP::to_num::<f32>(wh.y()) * 2.,
        3.,
        fill_color,
    );
}
#[derive(Clone, Copy, Debug)]
enum TagType {
    Tile,
    Player,
}
