use minifb::{Key, Scale, ScaleMode, Window, WindowOptions};
use png::{Decoder, Transformations};
use std::fs::File;

#[derive(Debug)]
struct Sprite {
    width: usize,
    height: usize,
    argb: Vec<u32>,
}

fn png_to_sprite(filename: &str) -> Sprite {
    let mut decoder = Decoder::new(File::open(filename).unwrap());
    decoder.set_transformations(Transformations::ALPHA);
    let mut reader = decoder.read_info().unwrap();
    let info = reader.info();
    println!("indexed level: {:?}", info);
    println!(
        "tiles: {}, buffer size in bytes: {}",
        info.width * info.height,
        reader.output_buffer_size()
    );
    let mut buffer = vec![0u32; reader.output_buffer_size() / 4];

    let u8_buffer = unsafe {
        std::slice::from_raw_parts_mut(
            buffer.as_mut_ptr() as *mut u8,
            buffer.len() * std::mem::size_of::<u32>(),
        )
    };
    reader.next_frame(u8_buffer).unwrap();

    let mut argb_buffer = Vec::new();
    // convert RGBA buffer read by the reader to an ARGB buffer as expected by minifb.
    // for (rgba, _argb) in u8_buffer.chunks_mut(4).zip(buffer.iter_mut()) {
    for rgba in u8_buffer.chunks_mut(4) {
        // extracting the subpixels
        let r = rgba[0] as u32;
        let g = rgba[1] as u32;
        let b = rgba[2] as u32;
        let a = rgba[3] as u32;

        // merging the subpixels in ARGB format.
        // *argb = a << 24 | r << 16 | g << 8 | b;
        argb_buffer.push(a << 24 | r << 16 | g << 8 | b);
    }
    let width = reader.info().width as usize;
    let height = reader.info().height as usize;
    Sprite {
        width,
        height,
        argb: argb_buffer,
    }
}

fn draw_pixel(sprite: &mut Sprite, x: i32, y: i32, color: u32) {
    if x < 0 || y < 0 {
        return;
    }
    if x >= sprite.width as i32 || y >= sprite.height as i32 {
        return;
    }
    let dest_ind = (y * sprite.width as i32 + x) as usize;
    sprite.argb[dest_ind] = color;
}

// draw sprite into buffer at location
fn draw_sprite(sprite: &Sprite, sprite_x: i32, sprite_y: i32, screen: &mut Sprite) {
    for sy in 0..sprite.height as i32 {
        let y = sprite_y + sy;
        if y < 0 {
            continue;
        }
        if y >= screen.height as i32 {
            break;
        }
        for sx in 0..sprite.width as i32 {
            let x = sprite_x + sx;
            if x < 0 {
                continue;
            }
            if x >= screen.width as i32 {
                break;
            }
            let src_ind = (sy * sprite.width as i32 + sx) as usize;
            let dest_ind = (y * screen.width as i32 + x) as usize;
            let src_pixel = sprite.argb[src_ind];
            // let dst_pixel = buffer[dest_ind];
            // TODO(lucasw) actually blend for intermediate alpha values
            let src_alpha = (src_pixel & 0xff000000) >> 24;
            if src_alpha == 0xff {
                screen.argb[dest_ind] = src_pixel;
            }
        }
    }
}

// TODO(lucasw) level struct
fn get_tile_type(
    level_bytes: &[u8],
    level_width: u32,
    level_height: u32,
    tile_x: u32,
    tile_y: u32,
) -> u8 {
    if tile_x >= level_width {
        return 0;
    }
    if tile_y >= level_height {
        return 0;
    }

    let orig_ind = tile_y * level_width + tile_x;
    let ind = (orig_ind / 8) as usize;
    let offset = 7 - (orig_ind % 8);
    let mask = 1 << offset;
    level_bytes[ind] & mask
}

fn get_tile_type_level_coords(
    level_bytes: &[u8],
    level_width: u32,
    level_height: u32,
    tile_width: usize,
    tile_height: usize,
    x: i32,
    y: i32,
) -> u8 {
    if x < 0 {
        return 0;
    }
    if y < 0 {
        return 0;
    }
    let tile_x = x as u32 / tile_width as u32;
    let tile_y = y as u32 / tile_height as u32;
    get_tile_type(level_bytes, level_width, level_height, tile_x, tile_y)
}

struct Level {
    width: u32,
    height: u32,
    tiles: Vec<u8>,
    wall_fg: Sprite,
    background: Sprite,
}

impl Level {
    fn new() -> Self {
        let wall_bg = png_to_sprite("data/wall_bg.png");
        let wall_fg = png_to_sprite("data/wall.png");

        let (level_bytes, level_width, level_height) = {
            // TODO(lucasw) this is a indexed png, don't want it as [u32] but not sure how to get it
            // let level = png_to_sprite("data/level00.png");
            // println!("{level:?}");
            let filename = "data/level00.png";
            let decoder = Decoder::new(File::open(filename).unwrap());
            // this causess indexed/paletted images to be expanded
            // decoder.set_transformations(Transformations::ALPHA);
            let mut reader = decoder.read_info().unwrap();
            let info = reader.info();
            // TODO(lucasw) check that the image actually is indexed
            println!("indexed level: {:?}", info);
            let width = info.width;
            let height = info.height;
            println!(
                "tiles: {}, buffer size: {}",
                info.width * info.height,
                reader.output_buffer_size()
            );
            let mut buffer = vec![0u8; reader.output_buffer_size()];
            // even though there is 1 bit-per-pixel this reads out into a u32 per pixel
            let rv = reader.next_frame(&mut buffer).unwrap();
            println!("{rv:?}");
            // println!("{buffer:?}");
            (buffer, width, height)
        };

        Level {
            width: level_width,
            height: level_height,
            tiles: level_bytes,
            wall_fg,
            background: wall_bg,
        }
    }

    fn is_collided(&self, test_x: i32, test_y: i32) -> bool {
        let tile_type = get_tile_type_level_coords(
            &self.tiles,
            self.width,
            self.height,
            self.wall_fg.width,
            self.wall_fg.height,
            test_x,
            test_y,
        );
        tile_type > 0
    }

    fn draw(&self, camera_x: i32, camera_y: i32, screen: &mut Sprite) {
        // TODO(lucasw) invert this, instead of looping over every tile in the map, loop over
        // every tile in the screen and look up in map
        for tile_y in 0..self.height {
            for tile_x in 0..self.width {
                let screen_x = tile_x as i32 * self.wall_fg.width as i32 - camera_x;
                if screen_x < -(self.wall_fg.width as i32) {
                    continue;
                }
                if screen_x > screen.width as i32 {
                    continue;
                }

                let screen_y = tile_y as i32 * self.wall_fg.height as i32 - camera_y;
                if screen_y < -(self.wall_fg.height as i32) {
                    continue;
                }
                if screen_y > screen.height as i32 {
                    continue;
                }

                let tile_type = get_tile_type(&self.tiles, self.width, self.height, tile_x, tile_y);
                let tile = {
                    if tile_type > 0 {
                        &self.wall_fg
                    } else {
                        &self.background
                    }
                };
                draw_sprite(tile, screen_x, screen_y, screen);
            }
        }
    }
}

struct Player {
    x: i32,
    // TODO(lucasw) making this float to be able to fall fractional pixels, but
    // it always needs to be rounded to nearest
    y: f64,
    sprite: Sprite,
    on_ground: bool,
    vy: f64,
    jump_pressed_prev: bool,
    viz_points: Vec<(i32, i32, u32)>,
}

impl Player {
    fn new(x: i32, y: i32) -> Self {
        let sprite = png_to_sprite("data/player.png");
        Player {
            x,
            y: y as f64,
            sprite,
            on_ground: false,
            vy: 0.0,
            jump_pressed_prev: false,
            viz_points: Vec::new(),
        }
    }

    fn update(
        &mut self,
        level: &Level,
        left_pressed: bool,
        right_pressed: bool,
        jump_pressed: bool,
    ) {
        self.viz_points.clear();

        if left_pressed ^ right_pressed {
            let x_step = 2;
            let mut actual_x_step;
            // TODO(lucasw) queue up collision points to test and zero out
            // left or right motion if any collide
            let test_x;
            if left_pressed {
                test_x = self.x + 4 - x_step;
                actual_x_step = -x_step;
            } else {
                // right_pressed
                // see if player has hit block moving to right
                test_x = self.x + self.sprite.width as i32 - 4 + x_step;
                actual_x_step = x_step;
            }
            for i in 0..4 {
                let test_y = (self.y as i32) + ((i + 1) * self.sprite.height / 5) as i32 - 1;
                let collided = level.is_collided(test_x, test_y);
                if collided {
                    println!("left collide");
                    actual_x_step = 0;
                    self.viz_points.push((test_x, test_y, 0x00ff0000));
                    break;
                }
                self.viz_points.push((test_x, test_y, 0x0011ff00));
            }
            self.x += actual_x_step;
        }

        // let min_x = -(self.width as i32);
        // let max_x = (screen.width + self.width) as i32;
        let min_x = 0;
        let max_x = (level.width as i32 - 1) * level.wall_fg.width as i32;
        if self.x > max_x {
            self.x = max_x;
        }
        if self.x < min_x {
            self.x = min_x;
        }

        let jump_pressed_rising = jump_pressed && !self.jump_pressed_prev;
        self.jump_pressed_prev = jump_pressed;

        if jump_pressed_rising && self.on_ground {
            println!("jump");
            // if this is too large the player can glitch through blocks, the collision response
            // just rounds to nearest block before the current collision
            self.vy = -16.0;
            // nudge the player off the ground so it doesn't immediately re-intersect
            self.y -= 2.0;
            self.on_ground = false;
        }
        /* TODO(lucasw) crouch
           if window.is_key_down(Key::Down) {
           self.y += 4;
        // println!("crouch");
        }
        */
        // println!("player {self.x} {self.y}, feet {test_x} {test_y} -> {tile_type} {self.on_ground}");

        if self.on_ground {
            self.vy = 0.0;
        } else {
            // println!("y pos {self.y}, vel {self.vy}");
            self.vy += 0.25;
            if self.vy > 4.0 {
                self.vy = 4.0;
            }
        }
        // self.y = self.y.round();

        // let min_y = -(self.height as i32);
        // let max_y = (screen.height + self.height) as i32;
        let min_y = 0.0;
        let max_y = (level.height as i32 * level.wall_fg.height as i32) as f64;
        if self.y > max_y {
            self.y = max_y;
            self.vy = 0.0;
            self.on_ground = true;
            println!("landed on bottom edge of level");
        }
        if self.y < min_y {
            self.y = min_y;
            self.vy = 0.0;
        }

        // see if player is on ground
        {
            let test_x = self.x + self.sprite.width as i32 / 2;
            let test_y = (self.y as i32) + self.sprite.height as i32;
            let collided = level.is_collided(test_x, test_y);
            if collided {
                self.viz_points.push((test_x, test_y, 0x00ff3300));
                if !self.on_ground {
                    println!("landed");
                    self.on_ground = true;
                    self.vy = 0.0;
                    let div = level.wall_fg.height as f64;
                    self.y = (self.y / div).round() * div;
                }
            } else if self.on_ground {
                self.viz_points.push((test_x, test_y, 0x0022ff33));
                println!("fell off edge");
                self.on_ground = false;
            }

            // TODO(lucasw) add test_x/y to debug viz queue
        }

        // see if player has hit head on tile
        {
            let test_x = self.x + self.sprite.width as i32 / 2;
            let start_y = self.y as i32;
            let end_y = (self.y + self.vy) as i32;
            // let div = level.wall_fg.height as f64;

            // TODO(lucasw) need same system for high speed falling
            let mut dy = 0.0;
            let step = 2;
            let mut range = Vec::new();
            if end_y >= start_y {
                for y in ((start_y + 1)..=end_y).step_by(step) {
                    range.push(y);
                }
            } else {
                for y in (end_y..=(start_y + 1)).rev().step_by(step) {
                    range.push(y);
                }
            }
            if !range.is_empty() {
                println!("{range:?}");
            }
            for test_y in range {
                let collided = level.is_collided(test_x, test_y - 1);
                println!("{} {}, {dy} {test_x} {test_y} {collided}", self.y, self.vy);
                if collided {
                    self.viz_points.push((test_x, test_y, 0x00ff3300));
                    if !self.on_ground {
                        println!("head bumped");
                        self.vy = 0.0;
                        break;
                    }
                } else {
                    dy = test_y as f64 - start_y as f64;
                    self.viz_points.push((test_x, test_y, 0x0022ff33));
                }
            }
            self.y += dy;
        }
    }

    fn draw(&self, camera_x: i32, camera_y: i32, dst_sprite: &mut Sprite) {
        draw_sprite(
            &self.sprite,
            self.x - camera_x,
            self.y as i32 - camera_y,
            dst_sprite,
        );

        // debug viz xy
        for (x, y, color) in &self.viz_points {
            let viz_screen_x = x - camera_x;
            let viz_screen_y = y - camera_y;
            draw_pixel(dst_sprite, viz_screen_x, viz_screen_y, *color);
        }
    }
}

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
        68 * level.wall_fg.width as i32,
        (level.height as i32 - 16) * level.wall_fg.height as i32,
    );
    // TODO(lucasw) wrap in struct

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
