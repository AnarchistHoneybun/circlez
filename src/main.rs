use clap::Parser;
use image::{ImageReader, RgbImage};
use minifb::{Key, Window, WindowOptions};
use rand::{random_range};
use std::path::{PathBuf, Path};

#[derive(Parser)]
struct Args {
    target: PathBuf,

    #[clap(short, long, default_value = "4096")]
    iterations: usize,
}

fn main() {
    let args = Args::parse();

    let target = ImageReader::open(&args.target)
        .expect("couldn't load given image")
        .decode()
        .expect("couldn't decode given image")
        .into_rgb8();

    let target = Image::from(target);
    let width = target.width;
    let height = target.height;

    let mut approx = Image::from(RgbImage::new(width, height));

    let mut canvas = vec![0; (width * height) as usize];

    let mut window = Window::new(
        "circlez",
        width as usize,
        height as usize,
        WindowOptions::default(),
    )
        .unwrap();

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let mut got_improvement = false;

        for _ in 0..args.iterations {
            got_improvement |= tick(&target, &mut approx);
        }

        if got_improvement {
            approx.encode(&mut canvas);
        }

        window
            .update_with_buffer(&canvas, width as usize, height as usize)
            .unwrap();
    }

    // Save the final image when window closes
    if !window.is_open() || window.is_key_down(Key::Escape) {
        // Create the output filename
        let input_path = Path::new(&args.target);
        let input_stem = input_path.file_stem().unwrap().to_str().unwrap();
        let output_filename = format!("generated_images/{}_circlez.jpg", input_stem);

        // Convert the current state to an image
        let mut output_image = RgbImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let [r, g, b] = approx.color_at([x, y]);
                output_image.put_pixel(x, y, image::Rgb([r, g, b]));
            }
        }

        // Ensure the directory exists
        std::fs::create_dir_all("generated_images").expect("Failed to create output directory");

        // Save the image
        output_image.save(&output_filename).expect("Failed to save output image");
        println!("Saved final image to: {}", output_filename);
    }
}

fn calculate_weighted_color(target: &Image, center_x: isize, center_y: isize, radius: isize,
                            circle_points: &[[isize; 2]]) -> [u8; 3] {
    // Get center color
    let center_color = if center_x >= 0 && center_y >= 0
        && center_x < target.width as isize && center_y < target.height as isize {
        target.color_at([center_x as u32, center_y as u32])
    } else {
        [0, 0, 0]
    };

    // Calculate average edge color from valid points
    let mut valid_points = 0;
    let edge_color = circle_points.iter()
        .filter(|[x, y]| {
            x >= &0 && y >= &0 &&
                x < &(target.width as isize) && y < &(target.height as isize)
        })
        .map(|[x, y]| {
            valid_points += 1;
            target.color_at([*x as u32, *y as u32])
        })
        .fold([0f32; 3], |acc, [r, g, b]| {
            [acc[0] + r as f32, acc[1] + g as f32, acc[2] + b as f32]
        });

    if valid_points == 0 {
        return center_color;
    }

    let edge_color = [
        (edge_color[0] / valid_points as f32) as u8,
        (edge_color[1] / valid_points as f32) as u8,
        (edge_color[2] / valid_points as f32) as u8,
    ];

    // Calculate weight based on radius (larger radius = more weight to edge color)
    let max_radius = (target.width.min(target.height) / 4) as f32;
    let weight = (radius as f32 / max_radius).min(1.0);

    // Blend colors
    [
        ((1.0 - weight) * center_color[0] as f32 + weight * edge_color[0] as f32) as u8,
        ((1.0 - weight) * center_color[1] as f32 + weight * edge_color[1] as f32) as u8,
        ((1.0 - weight) * center_color[2] as f32 + weight * edge_color[2] as f32) as u8,
    ]
}

fn tick(target: &Image, approx: &mut Image) -> bool {
    // Randomize center point
    let center_x = random_range(0..target.width) as isize;
    let center_y = random_range(0..target.height) as isize;

    // Randomize radius (limit to reasonable size based on image dimensions)
    let max_radius = (target.width.min(target.height) / 4) as isize;
    let radius = random_range(1..=max_radius as usize);

    // Generate circle points first so we can use them for both color calculation and drawing
    let circle_points = generate_circle_points(center_x, center_y, radius as isize);

    // Calculate weighted average color
    let color = calculate_weighted_color(target, center_x, center_y, radius as isize, &circle_points);

    // Generate all points that would be affected by the circle
    let changes = circle_points
        .into_iter()
        .filter(|&[x, y]| {
            x >= 0 &&
                y >= 0 &&
                x < target.width as isize &&
                y < target.height as isize
        })
        .map(|[x, y]| ([x as u32, y as u32], color));

    // Check if drawing this circle would improve the approximation
    let loss_delta = Image::loss_delta(target, approx, changes.clone());

    if loss_delta >= 0.0 {
        return false;
    }

    // Apply the changes if the circle improves the approximation
    approx.apply(changes);
    true
}

fn generate_circle_points(xc: isize, yc: isize, r: isize) -> Vec<[isize; 2]> {
    let mut points = Vec::new();
    let mut x = 0;
    let mut y = r;
    let mut d = 3 - 2 * r;

    while x <= y {
        // Add points in all octants
        let octant_points = [
            [xc + x, yc + y], [xc - x, yc + y],
            [xc + x, yc - y], [xc - x, yc - y],
            [xc + y, yc + x], [xc - y, yc + x],
            [xc + y, yc - x], [xc - y, yc - x],
        ];
        points.extend_from_slice(&octant_points);

        if d < 0 {
            d = d + 4 * x + 6;
        } else {
            d = d + 4 * (x - y) + 10;
            y -= 1;
        }
        x += 1;
    }
    points
}

type Point = [u32; 2];
type Color = [u8; 3];

struct Image {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Image {
    fn loss_delta(
        target: &Self,
        approx: &Self,
        changes: impl IntoIterator<Item = (Point, Color)>,
    ) -> f32 {
        changes
            .into_iter()
            .map(|(pos, new_col)| {
                let target_color = target.color_at(pos);
                let approx_color = approx.color_at(pos);

                let loss_without_changes = Self::pixel_loss(target_color, approx_color);
                let loss_with_changes = Self::pixel_loss(target_color, new_col);

                loss_with_changes - loss_without_changes
            })
            .sum()
    }

    fn pixel_loss(a: Color, b: Color) -> f32 {
        a.into_iter()
            .zip(b)
            .map(|(a, b)| (a as f32 - b as f32).powi(2))
            .sum()
    }

    fn apply(&mut self, changes: impl IntoIterator<Item = (Point, Color)>) {
        for (pos, col) in changes {
            *self.color_at_mut(pos) = col;
        }
    }

    fn encode(&self, buf: &mut [u32]) {
        let mut buf = buf.iter_mut();

        for y in 0..self.height {
            for x in 0..self.width {
                let [r, g, b] = self.color_at([x, y]);
                *buf.next().unwrap() = u32::from_be_bytes([0, r, g, b]);
            }
        }
    }

    fn color_at(&self, point: Point) -> Color {
        let offset = (point[1] * self.width + point[0]) as usize * 3;
        let color = &self.pixels[offset..][..3];
        color.try_into().unwrap()
    }

    fn color_at_mut(&mut self, [x, y]: [u32; 2]) -> &mut Color {
        let offset = (y * self.width + x) as usize * 3;
        let color = &mut self.pixels[offset..][..3];
        color.try_into().unwrap()
    }
}

impl From<RgbImage> for Image {
    fn from(img: RgbImage) -> Self {
        let width = img.width();
        let height = img.height();
        let pixels = img.pixels().flat_map(|pixel| pixel.0).collect();

        Self {
            width,
            height,
            pixels,
        }
    }
}