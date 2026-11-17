// SPDX-License-Identifier: GPL-3.0-only

//! Draws box-drawing and block characters as solid shapes that fill the
//! whole cell, instead of using the font's glyphs.
//!
//! Font glyphs for these characters only cover the font's own ascent and
//! descent. Terminal rows are taller than that (the line height is 1.4x the
//! font size), so boxes drawn with `│` or `█` would show gaps between rows.
//! This is the same approach Kitty, WezTerm, Alacritty and Ghostty use.

use cosmic::{
    iced::{
        Background, Border, Color, Point, Radians, Rectangle, Size, Vector,
        advanced::renderer::{self, Quad},
        border::Radius,
    },
    widget::canvas::{Frame, LineCap, LineJoin, Path, Stroke, path::arc},
};
use std::f32::consts::{FRAC_PI_2, PI};

/// Whether `c` is drawn by [`draw`] or [`draw_powerline`] rather than by the
/// font.
pub fn is_custom(c: char) -> bool {
    line_arms(c).is_some() || arc_corner(c).is_some() || blocks(c).is_some() || is_powerline(c)
}

/// Powerline separators (U+E0B0..U+E0BF), drawn with [`draw_powerline`].
///
/// Prompts like Oh My Posh and Starship put these right next to colored
/// backgrounds, so they must cover the full cell height or the "pill" and
/// arrow ends look shorter than the segment they belong to.
pub fn is_powerline(c: char) -> bool {
    ('\u{e0b0}'..='\u{e0bf}').contains(&c)
}

/// Add the shape for powerline character `c` filling `cell` to `frame`.
pub fn draw_powerline(frame: &mut Frame, c: char, cell: Rectangle, color: Color, font_size: f32) {
    let g = Grid::new(cell);
    let (l, r, t, b) = (g.left, g.right, g.top, g.bottom);
    let mid = (t + b) / 2.0;
    let p = Point::new;
    let polygon = |points: &[Point]| {
        Path::new(|builder| {
            builder.move_to(points[0]);
            for &point in &points[1..] {
                builder.line_to(point);
            }
            builder.close();
        })
    };
    let polyline = |points: &[Point]| {
        Path::new(|builder| {
            builder.move_to(points[0]);
            for &point in &points[1..] {
                builder.line_to(point);
            }
        })
    };
    // Half of an ellipse spanning the cell height, with its flat side on
    // `flat_x` and bulging to the right (`dir` = 1) or left (`dir` = -1)
    let half_ellipse = |flat_x: f32, dir: f32, closed: bool| {
        let start = if dir > 0.0 { -FRAC_PI_2 } else { FRAC_PI_2 };
        Path::new(|builder| {
            builder.ellipse(arc::Elliptical {
                center: p(flat_x, mid),
                radii: Vector::new(r - l, (b - t) / 2.0),
                rotation: Radians(0.0),
                start_angle: Radians(start),
                end_angle: Radians(start + PI),
            });
            if closed {
                builder.close();
            }
        })
    };

    let line_width = (font_size / 14.0).round().max(1.0);
    let stroke = Stroke::default()
        .with_color(color)
        .with_width(line_width)
        .with_line_cap(LineCap::Round)
        .with_line_join(LineJoin::Round);
    // Outlines are inset by half the line width so they stay in the cell
    let i = line_width / 2.0;
    // Solid shapes overlap the segment they continue by a pixel on their flat
    // side, so the anti-aliased edge never leaves a seam against the
    // neighbouring cell's background.
    let o = 1.0;

    match c {
        // Solid triangles pointing right and left
        '\u{e0b0}' => frame.fill(
            &polygon(&[p(l - o, t), p(l, t), p(r, mid), p(l, b), p(l - o, b)]),
            color,
        ),
        '\u{e0b2}' => frame.fill(
            &polygon(&[p(r + o, t), p(r, t), p(l, mid), p(r, b), p(r + o, b)]),
            color,
        ),
        // Thin chevrons
        '\u{e0b1}' => frame.stroke(
            &polyline(&[p(l, t + i), p(r - i, mid), p(l, b - i)]),
            stroke,
        ),
        '\u{e0b3}' => frame.stroke(
            &polyline(&[p(r, t + i), p(l + i, mid), p(r, b - i)]),
            stroke,
        ),
        // Solid and outline half circles (the "pill" caps)
        '\u{e0b4}' => {
            frame.fill(&half_ellipse(l, 1.0, true), color);
            frame.fill_rectangle(p(l - o, t), Size::new(o, b - t), color);
        }
        '\u{e0b6}' => {
            frame.fill(&half_ellipse(r, -1.0, true), color);
            frame.fill_rectangle(p(r, t), Size::new(o, b - t), color);
        }
        '\u{e0b5}' => frame.stroke(&half_ellipse(l, 1.0, false), stroke),
        '\u{e0b7}' => frame.stroke(&half_ellipse(r, -1.0, false), stroke),
        // Corner triangles and slanted separators
        '\u{e0b8}' => frame.fill(&polygon(&[p(l, t), p(r, b), p(l, b)]), color), // lower left
        '\u{e0ba}' => frame.fill(&polygon(&[p(r, t), p(r, b), p(l, b)]), color), // lower right
        '\u{e0bc}' => frame.fill(&polygon(&[p(l, t), p(r, t), p(l, b)]), color), // upper left
        '\u{e0be}' => frame.fill(&polygon(&[p(l, t), p(r, t), p(r, b)]), color), // upper right
        '\u{e0b9}' | '\u{e0bf}' => frame.stroke(&polyline(&[p(l, t), p(r, b)]), stroke), // \\
        '\u{e0bb}' | '\u{e0bd}' => frame.stroke(&polyline(&[p(l, b), p(r, t)]), stroke), // /
        _ => {}
    }
}

/// Draw `c` filling `cell` (in screen coordinates). Returns false if `c` is not
/// handled here.
pub fn draw<R: renderer::Renderer>(
    renderer: &mut R,
    c: char,
    cell: Rectangle,
    color: Color,
    font_size: f32,
) -> bool {
    let light = (font_size / 14.0).round().max(1.0);
    let heavy = light * 2.0;

    if let Some(arms) = line_arms(c) {
        draw_lines(renderer, cell, color, arms, light, heavy);
        return true;
    }

    if let Some(corner) = arc_corner(c) {
        draw_arc(renderer, cell, color, corner, light);
        return true;
    }

    if let Some((rects, alpha)) = blocks(c) {
        let color = Color {
            a: color.a * alpha,
            ..color
        };
        for &(x0, y0, x1, y1) in rects {
            // Snap to whole pixels so neighbouring cells meet without seams
            let left = (cell.x + cell.width * x0).round();
            let right = (cell.x + cell.width * x1).round();
            let top = (cell.y + cell.height * y0).round();
            let bottom = (cell.y + cell.height * y1).round();
            fill(renderer, left, top, right - left, bottom - top, color);
        }
        return true;
    }

    false
}

fn fill<R: renderer::Renderer>(renderer: &mut R, x: f32, y: f32, w: f32, h: f32, color: Color) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    renderer.fill_quad(
        Quad {
            bounds: Rectangle::new(Point::new(x, y), Size::new(w, h)),
            ..Default::default()
        },
        Background::Color(color),
    );
}

/// Pixel-snapped edges of the cell and of a centered line of `thickness`.
struct Grid {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

impl Grid {
    fn new(cell: Rectangle) -> Self {
        Self {
            left: cell.x.round(),
            right: (cell.x + cell.width).round(),
            top: cell.y.round(),
            bottom: (cell.y + cell.height).round(),
        }
    }

    /// Left edge of a centered vertical line of thickness `t`
    fn vx(&self, t: f32) -> f32 {
        ((self.left + self.right - t) / 2.0).floor()
    }

    /// Top edge of a centered horizontal line of thickness `t`
    fn hy(&self, t: f32) -> f32 {
        ((self.top + self.bottom - t) / 2.0).floor()
    }
}

fn draw_lines<R: renderer::Renderer>(
    renderer: &mut R,
    cell: Rectangle,
    color: Color,
    [up, right, down, left]: [u8; 4],
    light: f32,
    heavy: f32,
) {
    let g = Grid::new(cell);
    let thickness = |weight: u8| match weight {
        0 => 0.0,
        1 => light,
        _ => heavy,
    };
    // Lines reach past the center by half of the thickest crossing line so
    // corners and junctions are filled in.
    let horizontal_t = thickness(left).max(thickness(right));
    let vertical_t = thickness(up).max(thickness(down));

    if left > 0 {
        let t = thickness(left);
        let end = g.vx(vertical_t) + vertical_t;
        fill(renderer, g.left, g.hy(t), end - g.left, t, color);
    }
    if right > 0 {
        let t = thickness(right);
        let start = if vertical_t > 0.0 {
            g.vx(vertical_t)
        } else {
            g.vx(t)
        };
        fill(renderer, start, g.hy(t), g.right - start, t, color);
    }
    if up > 0 {
        let t = thickness(up);
        let end = g.hy(horizontal_t) + horizontal_t;
        fill(renderer, g.vx(t), g.top, t, end - g.top, color);
    }
    if down > 0 {
        let t = thickness(down);
        let start = if horizontal_t > 0.0 {
            g.hy(horizontal_t)
        } else {
            g.hy(t)
        };
        fill(renderer, g.vx(t), start, t, g.bottom - start, color);
    }
}

/// Which way a rounded corner opens, by the two arms it connects.
#[derive(Clone, Copy)]
enum Corner {
    DownRight,
    DownLeft,
    UpLeft,
    UpRight,
}

fn arc_corner(c: char) -> Option<Corner> {
    Some(match c {
        '\u{256d}' => Corner::DownRight, // ╭
        '\u{256e}' => Corner::DownLeft,  // ╮
        '\u{256f}' => Corner::UpLeft,    // ╯
        '\u{2570}' => Corner::UpRight,   // ╰
        _ => return None,
    })
}

/// Draw a rounded corner as one corner of a large bordered rounded
/// rectangle, clipped to the cell.
fn draw_arc<R: renderer::Renderer>(
    renderer: &mut R,
    cell: Rectangle,
    color: Color,
    corner: Corner,
    t: f32,
) {
    let g = Grid::new(cell);
    let vx = g.vx(t);
    let hy = g.hy(t);
    let radius = ((g.right - g.left) / 2.0).max(t);
    // Big enough that the rectangle's other corners are outside the cell
    let size = (g.right - g.left).max(g.bottom - g.top) * 2.0;

    let (x, y, mut r) = match corner {
        Corner::DownRight => (vx, hy, Radius::default()),
        Corner::DownLeft => (vx + t - size, hy, Radius::default()),
        Corner::UpLeft => (vx + t - size, hy + t - size, Radius::default()),
        Corner::UpRight => (vx, hy + t - size, Radius::default()),
    };
    match corner {
        Corner::DownRight => r.top_left = radius,
        Corner::DownLeft => r.top_right = radius,
        Corner::UpLeft => r.bottom_right = radius,
        Corner::UpRight => r.bottom_left = radius,
    }

    let clip = Rectangle::new(
        Point::new(g.left, g.top),
        Size::new(g.right - g.left, g.bottom - g.top),
    );
    renderer.with_layer(clip, |renderer| {
        renderer.fill_quad(
            Quad {
                bounds: Rectangle::new(Point::new(x, y), Size::new(size, size)),
                border: Border {
                    color,
                    width: t,
                    radius: r,
                },
                ..Default::default()
            },
            Background::Color(Color::TRANSPARENT),
        );
    });
}

/// Block elements (U+2580..U+259F) as rectangles in fractions of the cell,
/// plus an alpha for the shade characters.
fn blocks(c: char) -> Option<(&'static [(f32, f32, f32, f32)], f32)> {
    const E: f32 = 1.0 / 8.0;
    Some(match c {
        '\u{2580}' => (&[(0.0, 0.0, 1.0, 0.5)], 1.0), // ▀ upper half
        '\u{2581}' => (&[(0.0, 1.0 - E, 1.0, 1.0)], 1.0),
        '\u{2582}' => (&[(0.0, 1.0 - 2.0 * E, 1.0, 1.0)], 1.0),
        '\u{2583}' => (&[(0.0, 1.0 - 3.0 * E, 1.0, 1.0)], 1.0),
        '\u{2584}' => (&[(0.0, 0.5, 1.0, 1.0)], 1.0), // ▄ lower half
        '\u{2585}' => (&[(0.0, 1.0 - 5.0 * E, 1.0, 1.0)], 1.0),
        '\u{2586}' => (&[(0.0, 1.0 - 6.0 * E, 1.0, 1.0)], 1.0),
        '\u{2587}' => (&[(0.0, 1.0 - 7.0 * E, 1.0, 1.0)], 1.0),
        '\u{2588}' => (&[(0.0, 0.0, 1.0, 1.0)], 1.0), // █ full block
        '\u{2589}' => (&[(0.0, 0.0, 7.0 * E, 1.0)], 1.0),
        '\u{258a}' => (&[(0.0, 0.0, 6.0 * E, 1.0)], 1.0),
        '\u{258b}' => (&[(0.0, 0.0, 5.0 * E, 1.0)], 1.0),
        '\u{258c}' => (&[(0.0, 0.0, 0.5, 1.0)], 1.0), // ▌ left half
        '\u{258d}' => (&[(0.0, 0.0, 3.0 * E, 1.0)], 1.0),
        '\u{258e}' => (&[(0.0, 0.0, 2.0 * E, 1.0)], 1.0),
        '\u{258f}' => (&[(0.0, 0.0, E, 1.0)], 1.0),
        '\u{2590}' => (&[(0.5, 0.0, 1.0, 1.0)], 1.0), // ▐ right half
        '\u{2591}' => (&[(0.0, 0.0, 1.0, 1.0)], 0.25), // ░
        '\u{2592}' => (&[(0.0, 0.0, 1.0, 1.0)], 0.5), // ▒
        '\u{2593}' => (&[(0.0, 0.0, 1.0, 1.0)], 0.75), // ▓
        '\u{2594}' => (&[(0.0, 0.0, 1.0, E)], 1.0),   // ▔ upper eighth
        '\u{2595}' => (&[(1.0 - E, 0.0, 1.0, 1.0)], 1.0), // ▕ right eighth
        // Quadrants
        '\u{2596}' => (&[LL], 1.0),
        '\u{2597}' => (&[LR], 1.0),
        '\u{2598}' => (&[UL], 1.0),
        '\u{2599}' => (&[UL, LL, LR], 1.0),
        '\u{259a}' => (&[UL, LR], 1.0),
        '\u{259b}' => (&[UL, UR, LL], 1.0),
        '\u{259c}' => (&[UL, UR, LR], 1.0),
        '\u{259d}' => (&[UR], 1.0),
        '\u{259e}' => (&[UR, LL], 1.0),
        '\u{259f}' => (&[UR, LL, LR], 1.0),
        _ => return None,
    })
}

const UL: (f32, f32, f32, f32) = (0.0, 0.0, 0.5, 0.5);
const UR: (f32, f32, f32, f32) = (0.5, 0.0, 1.0, 0.5);
const LL: (f32, f32, f32, f32) = (0.0, 0.5, 0.5, 1.0);
const LR: (f32, f32, f32, f32) = (0.5, 0.5, 1.0, 1.0);

/// Generated from Unicode character names: [up, right, down, left], 1 = light, 2 = heavy
fn line_arms(c: char) -> Option<[u8; 4]> {
    Some(match c {
        '\u{2500}' => [0, 1, 0, 1], // light horizontal
        '\u{2501}' => [0, 2, 0, 2], // heavy horizontal
        '\u{2502}' => [1, 0, 1, 0], // light vertical
        '\u{2503}' => [2, 0, 2, 0], // heavy vertical
        '\u{250c}' => [0, 1, 1, 0], // light down and right
        '\u{250d}' => [0, 2, 1, 0], // down light and right heavy
        '\u{250e}' => [0, 1, 2, 0], // down heavy and right light
        '\u{250f}' => [0, 2, 2, 0], // heavy down and right
        '\u{2510}' => [0, 0, 1, 1], // light down and left
        '\u{2511}' => [0, 0, 1, 2], // down light and left heavy
        '\u{2512}' => [0, 0, 2, 1], // down heavy and left light
        '\u{2513}' => [0, 0, 2, 2], // heavy down and left
        '\u{2514}' => [1, 1, 0, 0], // light up and right
        '\u{2515}' => [1, 2, 0, 0], // up light and right heavy
        '\u{2516}' => [2, 1, 0, 0], // up heavy and right light
        '\u{2517}' => [2, 2, 0, 0], // heavy up and right
        '\u{2518}' => [1, 0, 0, 1], // light up and left
        '\u{2519}' => [1, 0, 0, 2], // up light and left heavy
        '\u{251a}' => [2, 0, 0, 1], // up heavy and left light
        '\u{251b}' => [2, 0, 0, 2], // heavy up and left
        '\u{251c}' => [1, 1, 1, 0], // light vertical and right
        '\u{251d}' => [1, 2, 1, 0], // vertical light and right heavy
        '\u{251e}' => [2, 1, 1, 0], // up heavy and right down light
        '\u{251f}' => [1, 1, 2, 0], // down heavy and right up light
        '\u{2520}' => [2, 1, 2, 0], // vertical heavy and right light
        '\u{2521}' => [2, 2, 1, 0], // down light and right up heavy
        '\u{2522}' => [1, 2, 2, 0], // up light and right down heavy
        '\u{2523}' => [2, 2, 2, 0], // heavy vertical and right
        '\u{2524}' => [1, 0, 1, 1], // light vertical and left
        '\u{2525}' => [1, 0, 1, 2], // vertical light and left heavy
        '\u{2526}' => [2, 0, 1, 1], // up heavy and left down light
        '\u{2527}' => [1, 0, 2, 1], // down heavy and left up light
        '\u{2528}' => [2, 0, 2, 1], // vertical heavy and left light
        '\u{2529}' => [2, 0, 1, 2], // down light and left up heavy
        '\u{252a}' => [1, 0, 2, 2], // up light and left down heavy
        '\u{252b}' => [2, 0, 2, 2], // heavy vertical and left
        '\u{252c}' => [0, 1, 1, 1], // light down and horizontal
        '\u{252d}' => [0, 1, 1, 2], // left heavy and right down light
        '\u{252e}' => [0, 2, 1, 1], // right heavy and left down light
        '\u{252f}' => [0, 2, 1, 2], // down light and horizontal heavy
        '\u{2530}' => [0, 1, 2, 1], // down heavy and horizontal light
        '\u{2531}' => [0, 1, 2, 2], // right light and left down heavy
        '\u{2532}' => [0, 2, 2, 1], // left light and right down heavy
        '\u{2533}' => [0, 2, 2, 2], // heavy down and horizontal
        '\u{2534}' => [1, 1, 0, 1], // light up and horizontal
        '\u{2535}' => [1, 1, 0, 2], // left heavy and right up light
        '\u{2536}' => [1, 2, 0, 1], // right heavy and left up light
        '\u{2537}' => [1, 2, 0, 2], // up light and horizontal heavy
        '\u{2538}' => [2, 1, 0, 1], // up heavy and horizontal light
        '\u{2539}' => [2, 1, 0, 2], // right light and left up heavy
        '\u{253a}' => [2, 2, 0, 1], // left light and right up heavy
        '\u{253b}' => [2, 2, 0, 2], // heavy up and horizontal
        '\u{253c}' => [1, 1, 1, 1], // light vertical and horizontal
        '\u{253d}' => [1, 1, 1, 2], // left heavy and right vertical light
        '\u{253e}' => [1, 2, 1, 1], // right heavy and left vertical light
        '\u{253f}' => [1, 2, 1, 2], // vertical light and horizontal heavy
        '\u{2540}' => [2, 1, 1, 1], // up heavy and down horizontal light
        '\u{2541}' => [1, 1, 2, 1], // down heavy and up horizontal light
        '\u{2542}' => [2, 1, 2, 1], // vertical heavy and horizontal light
        '\u{2543}' => [2, 1, 1, 2], // left up heavy and right down light
        '\u{2544}' => [2, 2, 1, 1], // right up heavy and left down light
        '\u{2545}' => [1, 1, 2, 2], // left down heavy and right up light
        '\u{2546}' => [1, 2, 2, 1], // right down heavy and left up light
        '\u{2547}' => [2, 2, 1, 2], // down light and up horizontal heavy
        '\u{2548}' => [1, 2, 2, 2], // up light and down horizontal heavy
        '\u{2549}' => [2, 1, 2, 2], // right light and left vertical heavy
        '\u{254a}' => [2, 2, 2, 1], // left light and right vertical heavy
        '\u{254b}' => [2, 2, 2, 2], // heavy vertical and horizontal
        '\u{2574}' => [0, 0, 0, 1], // light left
        '\u{2575}' => [1, 0, 0, 0], // light up
        '\u{2576}' => [0, 1, 0, 0], // light right
        '\u{2577}' => [0, 0, 1, 0], // light down
        '\u{2578}' => [0, 0, 0, 2], // heavy left
        '\u{2579}' => [2, 0, 0, 0], // heavy up
        '\u{257a}' => [0, 2, 0, 0], // heavy right
        '\u{257b}' => [0, 0, 2, 0], // heavy down
        '\u{257c}' => [0, 2, 0, 1], // light left and heavy right
        '\u{257d}' => [1, 0, 2, 0], // light up and heavy down
        '\u{257e}' => [0, 1, 0, 2], // heavy left and light right
        '\u{257f}' => [2, 0, 1, 0], // heavy up and light down
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_box_characters_are_custom() {
        for c in "─│┌┐└┘├┤┬┴┼━┃╭╮╯╰█▀▄░▒▓▌▐".chars()
        {
            assert!(is_custom(c), "{c:?} should be drawn by box_drawing");
        }
        for c in "\u{e0b0}\u{e0b4}\u{e0b6}\u{e0bf}".chars() {
            assert!(
                is_custom(c) && is_powerline(c),
                "{c:?} should be a powerline shape"
            );
        }
        for c in "a═║╔ \u{e0a0}\u{e0c0}".chars() {
            assert!(!is_custom(c), "{c:?} should be left to the font");
        }
    }

    #[test]
    fn line_arms_match_unicode_names() {
        assert_eq!(line_arms('─'), Some([0, 1, 0, 1]));
        assert_eq!(line_arms('│'), Some([1, 0, 1, 0]));
        assert_eq!(line_arms('┌'), Some([0, 1, 1, 0]));
        assert_eq!(line_arms('┼'), Some([1, 1, 1, 1]));
        // down light and right heavy
        assert_eq!(line_arms('┍'), Some([0, 2, 1, 0]));
        // light up and heavy down
        assert_eq!(line_arms('╽'), Some([1, 0, 2, 0]));
    }
}
