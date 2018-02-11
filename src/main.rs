#![feature(iterator_step_by, test)]

extern crate num;
extern crate minifb;
extern crate crossbeam;
extern crate simd;

use minifb::{Key, WindowOptions, Window};
use num::Complex;
use simd::{f32x4, u32x4};

const WIDTH:       usize = 1024;
const HEIGHT:      usize = 768;
const LIMIT:       u32   = 100;
const NUM_THREADS: usize = 8;
const COLORS: &'static [(f32, f32, f32)] = &[(0.0,    7.0,    100.0),
                                             (32.0,   107.0,  203.0),
                                             (237.0,  255.0,  255.0),
                                             (255.0,  170.0,  0.0),
                                             (0.0,    2.0,    0.0)];
                                             

fn pixel_to_point(bounds:      (usize, usize),
                  pixel:       (usize, usize),
                  upper_left:  Complex<f64>,
                  lower_right: Complex<f64>) -> Complex<f64> {

    let (width, height) = (lower_right.re - upper_left.re,
                           upper_left.im  - lower_right.im);
    Complex {
        re: upper_left.re + pixel.0 as f64 * width  / bounds.0 as f64,
        im: upper_left.im - pixel.1 as f64 * height / bounds.1 as f64
    }
}

#[inline(never)]
fn mandelbrot_vector(c_x: f32x4, c_y: f32x4, max_iter: u32) -> u32x4 {
    let mut x = c_x;
    let mut y = c_y;
    let mut count = u32x4::splat(0);
    for _ in 0..max_iter as usize {
        let xy = x * y;
        let xx = x * x;
        let yy = y * y;
        let sum = xx + yy;
        let mask = sum.lt(f32x4::splat(4.0));
        if !mask.any() { break }
        count = count + mask.to_i().select(u32x4::splat(1), u32x4::splat(0));
        x = xx - yy + c_x;
        y = xy + xy + c_y;
    }
    count
}

#[inline(never)]
fn render(pixels:      &mut [u32],
          bounds:      (usize, usize),
          upper_left:  Complex<f64>,
          lower_right: Complex<f64>) {

    assert!(pixels.len() == bounds.0 * bounds.1);

    let left             = upper_left.re  as f32;
    let right            = lower_right.re as f32;
    let top              = upper_left.im  as f32;
    let bottom           = lower_right.im as f32;
    let width_step:  f32 = (right - left) / WIDTH as f32;
    let height_step: f32 = (bottom - top) / (HEIGHT as f32 / NUM_THREADS as f32) ;
    let adjust           = f32x4::splat(width_step) * f32x4::new(0., 1., 2., 3.);

    for row in 0 .. bounds.1 {
        let y = f32x4::splat(top + height_step * row as f32);
        for column in (0 .. bounds.0).step_by(4) {
            let x = f32x4::splat(left + width_step * column as f32) + adjust;
            let points = mandelbrot_vector(x, y, LIMIT);            
            for k in 0..4 {
                let (r, g, b);
                let point_k = points.extract(k as u32);
                let val   = (point_k as f32 % 12.0) * (COLORS.len() as f32) / 12.0;
                let left  = val as usize % COLORS.len();
                let right = (left + 1) % COLORS.len();

                let p = val - left as f32;
                let (r1, g1, b1) = COLORS[left];
                let (r2, g2, b2) = COLORS[right];
                r = (r1 + (r2 - r1) * p) as u32;
                g = (g1 + (g2 - g1) * p) as u32;
                b = (b1 + (b2 - b1) * p) as u32;
                pixels[row * bounds.0 + column + k] = (r << 16) + (g << 8) + b
            }            
        }
    }
}

fn render_parallel(pixels:      &mut Vec<u32>,
                   bounds:      (usize, usize),
                   upper_left:  Complex<f64>,
                   lower_right: 
                   Complex<f64>) {

    let rows_per_band = bounds.1 / NUM_THREADS + 1;
    let bands: Vec<&mut [u32]> = pixels.chunks_mut(rows_per_band * bounds.0).collect();
    crossbeam::scope(|spawner| {
        for (i, band) in bands.into_iter().enumerate() {
            let top              = rows_per_band * i;
            let height           = band.len() / bounds.0;
            let band_bounds      = (bounds.0, height);
            let band_upper_left  = pixel_to_point(bounds, (0, top), upper_left, lower_right);
            let band_lower_right = pixel_to_point(bounds, (bounds.0, top + height), upper_left, lower_right);
            spawner.spawn(move || {
                render(band, band_bounds, band_upper_left, band_lower_right);
            });
        }
    });
}

fn main() {
    let mut buffer: Vec<u32> = vec![0; WIDTH * HEIGHT];
    let mut window = Window::new(
        "Sample RGBA32 buffer", WIDTH, HEIGHT, WindowOptions::default()
    ).unwrap_or_else(|e| { panic!("{}", e); });

    let mut upper_left  = Complex {re: -2.2, im:  1.0};
    let mut lower_right = Complex {re:  1.2, im: -1.0};
    let mut step        = 0.01;
    render_parallel(&mut buffer, (WIDTH, HEIGHT), upper_left, lower_right);
    while window.is_open() && !window.is_key_down(Key::Escape) {
        let mut need_update = false;
        window.get_keys().map(|keys| {
            for k in keys {
                match k {
                    Key::Left  => {upper_left.re -= step; lower_right.re -= step;},
                    Key::Right => {upper_left.re += step; lower_right.re += step;},
                    Key::Up    => {upper_left.im += step; lower_right.im += step;},
                    Key::Down  => {upper_left.im -= step; lower_right.im -= step;},
                    Key::W     => {upper_left.re /= 1.01; lower_right.re /= 1.01;
                                   upper_left.im /= 1.01; lower_right.im /= 1.01;
                                   step          /= 1.01;},
                    Key::S     => {upper_left.re *= 1.01; lower_right.re *= 1.01;
                                   upper_left.im *= 1.01; lower_right.im *= 1.01;
                                   step          *= 1.01},
                    _          => (),
                }
                need_update = true;
            }
            if need_update {
                render_parallel(&mut buffer, (WIDTH, HEIGHT), upper_left, lower_right);
                need_update = false;
            }
        });
        window.update_with_buffer(&buffer).unwrap();
    }
}
