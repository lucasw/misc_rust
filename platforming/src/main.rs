use minifb::{Key, Scale, ScaleMode, Window, WindowOptions};
use platforming::text::Text;
use platforming::{Level, Player, Sprite};

fn main() {
    let mut screen = Sprite {
        width: 320,
        height: 200,
        argb: vec![0u32; 320 * 200],
    };

    println!(
        "{} x {} = {} pixels",
        screen.width,
        screen.height,
        screen.argb.len()
    );

    let mut window = Window::new(
        "platform game",
        screen.width,
        screen.height,
        WindowOptions {
            resize: true,
            scale: Scale::X8,
            scale_mode: ScaleMode::AspectRatioStretch,
            ..WindowOptions::default()
        },
    )
    .expect("unable to create window");

    let level = Level::new();
    let mut player = Player::new(
        68 * level.tile_width() as i32,
        (level.height as i32 - 16) * level.tile_height() as i32,
    );
    // TODO(lucasw) wrap in struct

    let title_text = Text::new(screen.width, screen.height, 1);

    let fps = 60;
    window.set_target_fps(fps);
    let update_secs = 1.0 / fps as f64;
    // let update_duration = std::time::Duration::from_millis((update_secs * 1000.0) as u64);
    let update_duration = std::time::Duration::from_millis((update_secs * 1000.0 - 11.0) as u64);
    window.set_background_color(0, 0, 50);

    let mut prev = std::time::Instant::now();
    let mut count = 0;
    while window.is_open() && !window.is_key_down(Key::Escape) {
        let t0 = std::time::Instant::now();

        for elem in screen.argb.iter_mut() {
            *elem = 0u32;
        }
        // TODO(lucasw) have the camera lag behind the player
        let camera_x = player.x - (screen.width / 2) as i32;
        let camera_y = player.y as i32 - (screen.height / 2) as i32;

        level.draw(camera_x, camera_y, &mut screen);

        // update player and screen position
        {
            let left_pressed = window.is_key_down(Key::Left);
            let right_pressed = window.is_key_down(Key::Right);
            let jump_pressed = window.is_key_down(Key::Up);
            player.update(&level, left_pressed, right_pressed, jump_pressed);
            player.draw(camera_x, camera_y, &mut screen);
        }

        let t1 = std::time::Instant::now();
        let tdiff0 = t1 - t0;

        // TODO(lucasw) need to sleep for remaining time otherwise busywaiting for 1.0/fps in
        // window update?
        // No that just slows down the fps, not sure where all the cpu usage is happening,
        if update_duration > tdiff0 {
            let remaining_duration = update_duration - tdiff0;
            if count % 60 == 0 {
                println!("remaining {remaining_duration:?}, used {tdiff0:?}");
            }
            std::thread::sleep(remaining_duration);
        } else {
            println!("{tdiff0:?} > {update_duration:?}");
        }

        title_text.draw(&mut screen.argb, (3, 3), "platform");

        let t1 = std::time::Instant::now();

        // TODO(lucasw) this does appear to round up to remaining time, but also takes 10 ms or so
        // itself?  It appears to be the cost of copying the buffer, though it should be smaller
        // than 10ms
        window
            .update_with_buffer(&screen.argb, screen.width, screen.height)
            .unwrap();
        // window.update();

        let t2 = std::time::Instant::now();
        let tdiff1 = t2 - t1;
        if count % 60 == 0 {
            println!(
                "game update time: {tdiff0:?}, window update time: {tdiff1:?}, loop elapsed {:?}",
                t0 - prev
            );
        }
        prev = t0;
        count += 1;
    }
}
