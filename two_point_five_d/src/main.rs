use minifb::{Key, Window, WindowOptions};
use std::f64::consts::FRAC_PI_4 as PI4;

const WIDTH: usize = 640;
const HEIGHT: usize = 360;

fn small_pixel(
    map_buffer: &mut [u32],
    map_width: usize,
    map_sc: usize,
    yi: usize,
    xi: usize,
    color: u32,
) {
    let pi = (yi * map_sc) * (map_width * map_sc) + (xi * map_sc);
    map_buffer[pi] = color;
}

fn big_pixel(
    map_buffer: &mut [u32],
    map_width: usize,
    map_sc: usize,
    yi: usize,
    xi: usize,
    color: u32,
) {
    for ox in 0..map_sc {
        for oy in 0..map_sc {
            let pi = (yi * map_sc + oy) * (map_width * map_sc) + (xi * map_sc + ox);
            map_buffer[pi] = color;
        }
    }
}

fn main() {
    let num = WIDTH * HEIGHT;
    let mut buffer: Vec<u32> = vec![0; num];

    let map_width = 128;
    let map_height = 64;
    let mut map: Vec<u8> = vec![0; map_width * map_height];
    for i in 0..map_width {
        let x = i;
        let y = 8;
        map[y * map_width + x] = 0xff;
        let y = map_height - 8;
        map[y * map_width + x] = 0xff;
    }
    for i in 0..map_height {
        let x = 4;
        let y = i;
        map[y * map_width + x] = 0xff;
        let x = map_width - 12;
        map[y * map_width + x] = 0xff;
    }

    let map_sc = 4;
    let mut map_buffer: Vec<u32> = vec![0; map_width * map_sc * map_height * map_sc];
    let mut window = Window::new(
        "Test - ESC to exit",
        WIDTH,
        HEIGHT,
        WindowOptions::default(),
    )
    .unwrap_or_else(|e| {
        panic!("{}", e);
    });
    // Limit to max ~60 fps update rate
    window.set_target_fps(60);

    let mut map_window = Window::new(
        "Map",
        map_width * map_sc,
        map_height * map_sc,
        WindowOptions::default(),
    )
    .unwrap_or_else(|e| {
        panic!("{}", e);
    });
    map_window.set_target_fps(60);

    let mut count = 0;
    // let mut color = 0xffffffff;
    let color = 0x00ff0000;

    let mut x = map_width / 4;
    let mut y = map_height / 4;
    let mut view_angle = 0.0;
    // horizontal angle-of-view radians
    let h_aov = 2.0;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        for i in buffer.iter_mut() {
            // blank the image
            *i = 0;
        }

        if window.is_key_down(Key::Q) {
            view_angle += 0.006;
        }
        if window.is_key_down(Key::E) {
            view_angle -= 0.00595;
        }

        if window.is_key_down(Key::Left) && x > 0 && map[y * map_width + (x - 1)] == 0x0 {
            x -= 1;
        }
        if window.is_key_down(Key::Right)
            && x < (map_width - 1)
            && map[y * map_width + (x + 1)] == 0x0
        {
            x += 1;
        }
        if window.is_key_down(Key::Up) && y > 0 && map[(y - 1) * map_width + x] == 0x0 {
            y -= 1;
        }
        if window.is_key_down(Key::Down)
            && y < (map_height - 1)
            && map[(y + 1) * map_width + x] == 0x0
        {
            y += 1;
        }
        x %= map_width;
        y %= map_height;
        count += 1;
        if count % (60 * 2) == 0 {
            println!("x {x}, y {y}, view_angle {view_angle:.2}");
        }

        // clear and draw map walls
        for yi in 0..map_height {
            for xi in 0..map_width {
                let color = {
                    if map[yi * map_width + xi] != 0 {
                        0xff9f6054
                    } else {
                        0xff111511
                    }
                };

                big_pixel(&mut map_buffer, map_width, map_sc, yi, xi, color);
            }
        }

        // cast a ray for each pixel in a row
        // if count % 60 == 0 { println!("# "); }
        for i in 0..WIDTH {
            let i_fr = (i as f32 / WIDTH as f32) - 0.5;
            let ray_angle = view_angle + i_fr * h_aov / 2.0;
            // if count % 60 == 0 { print!("{ray_angle:.2} "); }

            let (collision_x, collision_y) = {
                let cos = ray_angle.cos();
                let sin = ray_angle.sin();
                // four quadrants
                if cos.abs() > PI4.cos() as f32 {
                    // facing up or down
                    let (xs, ys) = {
                        if cos > 0.0 {
                            (-sin / cos, -1.0)
                        } else {
                            (sin / cos, 1.0)
                        }
                    };

                    let mut collision_x = x as f32;
                    let mut collision_y = y as f32;
                    for _ in 0..90 {
                        let new_cx = collision_x + xs;
                        let new_cy = collision_y + ys;
                        let new_cxi = new_cx as usize;
                        let new_cyi = new_cy as usize;
                        if new_cxi >= map_width {
                            break;
                        }
                        if new_cyi >= map_height {
                            break;
                        }
                        collision_x = new_cx;
                        collision_y = new_cy;

                        small_pixel(
                            &mut map_buffer,
                            map_width,
                            map_sc,
                            new_cyi,
                            new_cxi,
                            0xff334495,
                        );

                        if map[collision_y as usize * map_width + collision_x as usize] != 0x0 {
                            break;
                        }
                    }

                    (collision_x, collision_y)
                // } if sin.abs() > PI4.sin() {
                } else {
                    // facing left or right
                    let (xs, ys) = {
                        if sin > 0.0 {
                            (-1.0, -cos / sin)
                        } else {
                            (1.0, cos / sin)
                        }
                    };

                    let mut collision_x = x as f32;
                    let mut collision_y = y as f32;
                    for _ in 0..90 {
                        let new_cx = collision_x + xs;
                        let new_cy = collision_y + ys;
                        let new_cxi = new_cx as usize;
                        let new_cyi = new_cy as usize;
                        if new_cxi >= map_width {
                            break;
                        }
                        if new_cyi >= map_height {
                            break;
                        }
                        collision_x = new_cx;
                        collision_y = new_cy;

                        small_pixel(
                            &mut map_buffer,
                            map_width,
                            map_sc,
                            new_cyi,
                            new_cxi,
                            0xff339435,
                        );

                        if map[collision_y as usize * map_width + collision_x as usize] != 0x0 {
                            break;
                        }
                    }

                    (collision_x, collision_y)
                }
            };

            let dx = x as f32 - collision_x;
            let dy = y as f32 - collision_y;
            let dist2 = (dx * dx + dy * dy).sqrt();

            let pix_height = (12.0 * (map_height / 2) as f32 / dist2) as usize;
            for j in 0..pix_height {
                let px = i;
                let py = HEIGHT / 2 + j;
                if py >= HEIGHT {
                    break;
                }
                buffer[py * WIDTH + px] = color;
            }
        }

        // We unwrap here as we want this code to exit if it fails. Real applications may want to handle this in a different way
        window.update_with_buffer(&buffer, WIDTH, HEIGHT).unwrap();

        // draw player position
        big_pixel(&mut map_buffer, map_width, map_sc, y, x, 0xffeebbdd);

        map_window
            .update_with_buffer(&map_buffer, map_width * map_sc, map_height * map_sc)
            .unwrap();
    }
}
