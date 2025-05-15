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
    println!("tiles: {}, buffer size in bytes: {}", info.width * info.height, reader.output_buffer_size());
    let mut buffer = vec![0u32; reader.output_buffer_size() / 4];

    let mut u8_buffer = unsafe {
        std::slice::from_raw_parts_mut(
            buffer.as_mut_ptr() as *mut u8,
            buffer.len() * std::mem::size_of::<u32>(),
        )
    };
    reader.next_frame(&mut u8_buffer).unwrap();

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
    sprite.argb[dest_ind] = 0xffee00ff;
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
fn get_tile_type(level_bytes: &[u8], level_width: u32, level_height: u32, tile_x: u32, tile_y: u32) -> u8 {
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

fn get_tile_type_level_coords(level_bytes: &[u8], level_width: u32, level_height: u32,
    tile_width: usize, tile_height: usize, x: i32, y: i32) -> u8 {
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

fn main() {
    let mut screen = Sprite {
        width: 320,
        height: 200,
        argb: vec![0u32; 320 * 200],
    };

    println!("{} x {} = {} pixels", screen.width, screen.height, screen.argb.len());

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

    let wall_bg = png_to_sprite("data/wall_bg.png");
    let wall_fg = png_to_sprite("data/wall.png");

    let (level_bytes, level_width, level_height) = {
        // TODO(lucasw) this is a indexed png, don't want it as [u32] but not sure how to get it
        // let level = png_to_sprite("data/level00.png");
        // println!("{level:?}");
        let filename = "data/level00.png";
        let mut decoder = Decoder::new(File::open(filename).unwrap());
        // this causess indexed/paletted images to be expanded
        // decoder.set_transformations(Transformations::ALPHA);
        let mut reader = decoder.read_info().unwrap();
        let info = reader.info();
        // TODO(lucasw) check that the image actually is indexed
        println!("indexed level: {:?}", info);
        let width = info.width;
        let height = info.height;
        println!("tiles: {}, buffer size: {}", info.width * info.height, reader.output_buffer_size());
        let mut buffer = vec![0u8; reader.output_buffer_size()];
        // even though there is 1 bit-per-pixel this reads out into a u32 per pixel
        let rv = reader.next_frame(&mut buffer).unwrap();
        println!("{rv:?}");
        // println!("{buffer:?}");
        (buffer, width, height)
    };

    // TODO(lucasw) wrap in struct
    let player = png_to_sprite("data/player.png");
    let mut player_x = 16 * wall_fg.width as i32;
    let mut player_y: f64 = ((level_height as i32 - 16) * wall_fg.height as i32) as f64;
    let mut player_on_ground = false;
    let mut player_vy = 0.0;

    window.set_target_fps(60);
    window.set_background_color(0, 0, 50);

    // let mut count = 0;
    while window.is_open() && !window.is_key_down(Key::Escape) {
        // count += 1;

        // TODO(lucasw) have the camera lag behind the player
        let camera_x = player_x - (screen.width / 2) as i32;
        let camera_y = player_y as i32 - (screen.height / 2) as i32;

        // draw the level
        {
            // TODO(lucasw) invert this, instead of looping over every tile in the map, loop over
            // every tile in the screen and look up in map
            for tile_y in 0..level_height {
                for tile_x in 0..level_width {
                    let screen_x = tile_x as i32 * wall_fg.width as i32 - camera_x as i32;
                    if screen_x < -(wall_fg.width as i32) {
                        continue;
                    }
                    if screen_x > screen.width as i32 {
                        continue;
                    }

                    let screen_y = tile_y as i32 * wall_fg.height as i32 - camera_y as i32;
                    if screen_y < -(wall_fg.height as i32) {
                        continue;
                    }
                    if screen_y > screen.height as i32 {
                        continue;
                    }

                    let tile_type = get_tile_type(&level_bytes, level_width, level_height, tile_x, tile_y);
                    let tile = {
                        if tile_type > 0 {
                            &wall_fg
                        } else {
                            &wall_bg
                        }
                    };
                    draw_sprite(tile, screen_x, screen_y, &mut screen);
                }
            }
        }

        // update player and screen position
        {
            {
                if window.is_key_down(Key::Left) {
                    player_x -= 2;
                    // println!("left");
                }
                if window.is_key_down(Key::Right) {
                    player_x += 2;
                    // println!("right");
                }
                // let min_x = -(player.width as i32);
                // let max_x = (screen.width + player.width) as i32;
                let min_x = 0;
                let max_x = level_width as i32 * wall_fg.width as i32;
                if player_x > max_x{
                    player_x = max_x;
                }
                if player_x < min_x {
                    player_x = min_x;
                }
            }

            {
                if window.is_key_down(Key::Up) {
                    if player_on_ground {
                        println!("jump");
                        player_vy = -8.0;
                        // nudge the player off the ground so it doesn't immediately re-intersect
                        player_y -= 2.0;
                        player_on_ground = false;
                    }
                    // println!("left");
                }
                /* TODO(lucasw) crouch
                if window.is_key_down(Key::Down) {
                    player_y += 4;
                    // println!("crouch");
                }
                */
                // println!("player {player_x} {player_y}, feet {test_x} {test_y} -> {tile_type} {player_on_ground}");

                if player_on_ground {
                    player_vy = 0.0;
                } else {
                    player_y += player_vy;
                    println!("y pos {player_y}, vel {player_vy}");
                    player_vy += 0.25;
                    if player_vy > 4.0 {
                        player_vy = 4.0;
                    }
                }
                // player_y = player_y.round();

                // let min_y = -(player.height as i32);
                // let max_y = (screen.height + player.height) as i32;
                let min_y = 0.0;
                let max_y = (level_height as i32 * wall_fg.height as i32) as f64;
                if player_y > max_y {
                    player_y = max_y;
                    player_vy = 0.0;
                    player_on_ground = true;
                    println!("landed on bottom edge of level");
                }
                if player_y < min_y {
                    player_y = min_y;
                    player_vy = 0.0;
                }

                let test_x = player_x + player.width as i32 / 2;
                let test_y = (player_y as i32) + player.height as i32;
                let tile_type = get_tile_type_level_coords(&level_bytes, level_width, level_height,
                    wall_fg.width, wall_fg.height, test_x, test_y);
                if tile_type > 0 {
                    if !player_on_ground {
                        println!("landed");
                        player_on_ground = true;
                        player_vy = 0.0;
                        let div = wall_fg.height as f64;
                        player_y = (player_y / div).round() * div;
                    }
                } else {
                    if player_on_ground {
                        println!("fell off edge");
                        player_on_ground = false;
                    }
                }

                draw_sprite(&player, player_x - camera_x, player_y as i32 - camera_y, &mut screen);

                // debug test xy
                {
                    let test_screen_x = test_x - camera_x;
                    let test_screen_y = test_y - camera_y;
                    draw_pixel(&mut screen, test_screen_x, test_screen_y, 0xffee00ff);
                }
            }
        }

        window.update_with_buffer(&screen.argb, screen.width, screen.height).unwrap();

        // TODO(lucasw) sleep for remaining time left in loop, or slightly less than that,
        // and cpu load will be reduced?
    }
}
