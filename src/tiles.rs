use crate::error::Result;
use crate::{draw_batch, game_state};
use nalgebra::{Point2, Point3, Vector2};
use slhack::utils;

use std::collections::HashMap;
use std::path::Path;

use allegro::*;
use allegro_font::*;

pub const TILE_SIZE: f32 = 32.;
pub const TILE_MAX_HEALTH: f32 = 100.;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TileKind
{
	Empty,
	Rock
	{
		health: f32,
		support: i32,
		intrinsic_support: bool,
		height: f32,
	},
	Torch,
	Support,
	Border,
	Grinder,
	Jaunter,
}

impl TileKind
{
	fn get_frame_and_height(&self, time: f64) -> (i32, f32)
	{
		match self
		{
			TileKind::Empty => (0, 0.0),
			TileKind::Border => (1, 0.0),
			TileKind::Rock { health, height, .. } =>
			{
				let num_tiles: i32 = 4;
				let f = health / TILE_MAX_HEALTH;
				(
					1 + utils::clamp(
						num_tiles - 1 - (f * num_tiles as f32 - 0.5).trunc() as i32,
						0,
						num_tiles - 1,
					),
					*height,
				)
			}
			TileKind::Torch => (5, 0.0),
			TileKind::Support => (6, 0.0),
			TileKind::Grinder => (7 + ((time * 3.) as i32 % 2), 0.),
			TileKind::Jaunter => (9 + ((time * 3.) as i32 % 2), 0.),
		}
	}

	pub fn is_solid(&self) -> bool
	{
		match self
		{
			TileKind::Empty | TileKind::Torch | TileKind::Support | TileKind::Jaunter => false,
			TileKind::Rock { .. } | TileKind::Border | TileKind::Grinder => true,
		}
	}
}

pub struct Tiles
{
	tiles: Vec<TileKind>,
	pub width: i32,
	pub height: i32,
}

impl Tiles
{
	pub fn new(width: i32, height: i32) -> Result<Self>
	{
		let mut tiles = vec![
			TileKind::Rock {
				health: TILE_MAX_HEALTH,
				support: 0,
				intrinsic_support: false,
				height: 0.,
			};
			(width * height) as usize
		];

		let center = Point2::new(10., 10.);
		for y in 0..height
		{
			for x in 0..width
			{
				let test_point = Point2::new(x, y).cast::<f32>();
				if (test_point - center).norm() < 5. || y == 10
				{
					tiles[y as usize * width as usize + x as usize] = TileKind::Empty;
				}
			}
		}

		for y in 0..height
		{
			for x in 0..width
			{
				if y < height - 1
				{
					let tile_idx = (y + 1) * width + x;
					let over_empty = matches!(tiles[tile_idx as usize], TileKind::Empty);
					let tile_idx = y * width + x;
					if let TileKind::Rock {
						intrinsic_support, ..
					} = &mut tiles[tile_idx as usize]
					{
						*intrinsic_support = over_empty;
					}
				}

				if x == 0 || x == width - 1 || y == 0 || y == height - 1
				{
					let tile_idx = y * width + x;
					tiles[tile_idx as usize] = TileKind::Border;
				}
			}
		}

		tiles[(width * (height - 1) + width / 2) as usize] = TileKind::Grinder;

		Ok(Self {
			tiles: tiles,
			width: width,
			height: height,
		})
	}

	fn get_tile_idx(&self, pos: Point2<f32>) -> Option<usize>
	{
		let tile_x = (pos.x / TILE_SIZE).floor() as i32;
		let tile_y = (pos.y / TILE_SIZE).floor() as i32;
		if tile_x < 0 || tile_x >= self.width || tile_y < 0 || tile_y >= self.height
		{
			None
		}
		else
		{
			Some(tile_y as usize * self.width as usize + tile_x as usize)
		}
	}

	pub fn get_tile_kind(&self, pos: Point2<f32>) -> TileKind
	{
		if let Some(idx) = self.get_tile_idx(pos)
		{
			self.tiles[idx]
		}
		else
		{
			TileKind::Empty
		}
	}

	pub fn get_tile_kind_mut(&mut self, pos: Point2<f32>) -> Option<&mut TileKind>
	{
		if let Some(idx) = self.get_tile_idx(pos)
		{
			Some(&mut self.tiles[idx])
		}
		else
		{
			None
		}
	}

	pub fn get_tiles_in_radius(
		&mut self, pos: Point2<f32>, radius: f32,
		mut callback_fn: impl FnMut(Point2<f32>, &mut TileKind),
	)
	{
		let tile_x = (pos.x / TILE_SIZE) as i32;
		let tile_y = (pos.y / TILE_SIZE) as i32;
		let tile_radius = (radius / TILE_SIZE).ceil() as i32;
		for map_y in tile_y - tile_radius..=tile_y + tile_radius
		{
			for map_x in tile_x - tile_radius..=tile_x + tile_radius
			{
				if map_x < 0 || map_x >= self.width || map_y < 0 || map_y >= self.height
				{
					continue;
				}
				let tile = &mut self.tiles[(map_y * self.width + map_x) as usize];
				let tile_center = Point2::new(
					map_x as f32 * TILE_SIZE + TILE_SIZE / 2.,
					map_y as f32 * TILE_SIZE + TILE_SIZE / 2.,
				);
				if (pos - tile_center).norm() < radius
				{
					callback_fn(tile_center, tile)
				}
			}
		}
	}

	pub fn get_next_jaunter(&self, pos: Point2<f32>) -> Option<Point2<f32>>
	{
		if let Some(cur_idx) = self.get_tile_idx(pos)
		{
			if self.tiles[cur_idx] != TileKind::Jaunter
			{
				return None;
			}
			for (offt_idx, tile) in self.tiles[cur_idx + 1..].iter().enumerate()
			{
				if *tile == TileKind::Jaunter
				{
					let tile_idx = cur_idx + 1 + offt_idx;
					let tile_x = tile_idx % self.width as usize;
					let tile_y = tile_idx / self.width as usize;
					return Some(Point2::new(
						tile_x as f32 * TILE_SIZE,
						tile_y as f32 * TILE_SIZE,
					));
				}
			}
			for (tile_idx, tile) in self.tiles[0..cur_idx].iter().enumerate()
			{
				if *tile == TileKind::Jaunter
				{
					let tile_x = tile_idx % self.width as usize;
					let tile_y = tile_idx / self.width as usize;
					return Some(Point2::new(
						tile_x as f32 * TILE_SIZE,
						tile_y as f32 * TILE_SIZE,
					));
				}
			}
			None
		}
		else
		{
			None
		}
	}

	/// size is radius.
	pub fn get_escape_dir(
		&self, pos: Point2<f32>, size: f32, avoid_fn: impl Fn(TileKind) -> bool,
	) -> Option<Vector2<f32>>
	{
		let tile_x = (pos.x / TILE_SIZE) as i32;
		let tile_y = (pos.y / TILE_SIZE) as i32;

		let mut res = Vector2::zeros();
		// TODO: This -1/1 isn't really right (???)
		for map_y in tile_y - 1..=tile_y + 1
		{
			for map_x in tile_x - 1..=tile_x + 1
			{
				if map_x < 0 || map_x >= self.width || map_y < 0 || map_y >= self.height
				{
					continue;
				}
				let tile = self.tiles[(map_y * self.width + map_x) as usize];
				if !avoid_fn(tile)
				{
					continue;
				}

				let cx = map_x as f32 * TILE_SIZE;
				let cy = map_y as f32 * TILE_SIZE;

				let vs = [
					Point2::new(cx, cy),
					Point2::new(cx + TILE_SIZE, cy),
					Point2::new(cx + TILE_SIZE, cy + TILE_SIZE),
					Point2::new(cx, cy + TILE_SIZE),
				];

				let nearest_point = utils::nearest_poly_point(&vs, pos);

				let nearest_dist = utils::max(1e-20, (pos - nearest_point).norm());
				let inside = utils::is_inside_poly(&vs, pos);
				if nearest_dist < size || inside
				{
					let new_dir = if inside
					{
						(nearest_point - pos) * (nearest_dist + size) / nearest_dist
					}
					else
					{
						(pos - nearest_point) * (size - nearest_dist) / nearest_dist
					};

					if new_dir.norm() > res.norm()
					{
						res = new_dir;
					}
				}
			}
		}
		if res.norm() > 0. { Some(res) } else { None }
	}

	pub fn logic(
		&mut self, state: &mut game_state::GameState, camera_pos: Point2<f32>,
	) -> Result<Vec<Point2<f32>>>
	{
		let mut kill_pos = vec![];
		let solid_support = 3;
		for y in (0..self.height).rev()
		{
			for x in 0..self.width
			{
				let bottom_support = if y == self.height - 1
				{
					solid_support
				}
				else
				{
					let tile_idx = (y + 1) * self.width + x;
					match &self.tiles[tile_idx as usize]
					{
						TileKind::Rock { support, .. } => *support,
						TileKind::Support => solid_support,
						TileKind::Border => solid_support,
						_ => 0,
					}
				};

				let left_support = if x == 0
				{
					solid_support
				}
				else
				{
					let tile_idx = y * self.width + x - 1;
					match &self.tiles[tile_idx as usize]
					{
						TileKind::Rock { support, .. } => *support,
						TileKind::Support => solid_support,
						TileKind::Border => solid_support,
						_ => 0,
					}
				};

				let tile_idx = y * self.width + x;
				let tile = &mut self.tiles[tile_idx as usize];

				match tile
				{
					TileKind::Rock { support, .. } =>
					{
						*support = utils::max(left_support - 1, bottom_support);
					}
					_ => (),
				}
			}

			for x in (0..self.width).rev()
			{
				let right_support = if x == self.width - 1
				{
					solid_support
				}
				else
				{
					let tile_idx = y * self.width + x + 1;
					match &self.tiles[tile_idx as usize]
					{
						TileKind::Rock { support, .. } => *support,
						TileKind::Support => solid_support,
						TileKind::Border => solid_support,
						_ => 0,
					}
				};

				let bottom_left_support = if x == 0 || y == self.height - 1
				{
					solid_support
				}
				else
				{
					let tile_idx = (y + 1) * self.width + x - 1;
					match &self.tiles[tile_idx as usize]
					{
						TileKind::Rock { support, .. } => *support,
						TileKind::Border => solid_support,
						_ => 0,
					}
				};

				let mut num_supports = 0;
				if bottom_left_support > 0
				{
					num_supports += 1;
				}
				let bottom_right_support = if x == self.width - 1 || y == self.height - 1
				{
					solid_support
				}
				else
				{
					let tile_idx = (y + 1) * self.width + x + 1;
					match &self.tiles[tile_idx as usize]
					{
						TileKind::Rock { support, .. } => *support,
						TileKind::Border => solid_support,
						_ => 0,
					}
				};
				if bottom_right_support > 0
				{
					num_supports += 1;
				}

				let bonus_support = if num_supports == 2 { 1 } else { 0 };

				let tile_idx = y * self.width + x;
				let tile = &mut self.tiles[tile_idx as usize];

				match tile
				{
					TileKind::Rock {
						support,
						intrinsic_support,
						..
					} =>
					{
						*support = utils::clamp(
							utils::max(
								if *intrinsic_support
								{
									solid_support
								}
								else
								{
									*support
								},
								right_support - 1,
							) + bonus_support,
							0,
							solid_support,
						);
					}
					_ => (),
				}
			}
		}

		for y in (0..self.height - 1).rev()
		{
			for x in 0..self.width
			{
				let tile_idx = y * self.width + x;
				let health = if let TileKind::Rock {
					health,
					support,
					height,
					..
				} = &self.tiles[tile_idx as usize]
				{
					if *height == 0. && *support <= 0
					{
						Some(*health)
					}
					else
					{
						None
					}
				}
				else
				{
					None
				};

				if let Some(health) = health
				{
					let tile_idx = y * self.width + x;
					self.tiles[tile_idx as usize] = TileKind::Empty;
					let tile_idx = (y + 1) * self.width + x;

					let tile_pos = Point2::new(x as f32 * TILE_SIZE, (y + 1) as f32 * TILE_SIZE);

					state.sfx.play_positional_sound(
						"data/collapse_1.ogg",
						tile_pos,
						camera_pos,
						1.,
					)?;

					kill_pos.push(tile_pos);
					self.tiles[tile_idx as usize] = TileKind::Rock {
						health,
						support: 0,
						intrinsic_support: false,
						height: TILE_SIZE,
					};
				}
			}
		}

		for (tile_idx, tile) in self.tiles.iter_mut().enumerate()
		{
			let new_tile = match tile
			{
				TileKind::Rock {
					health,
					height,
					support,
					..
				} =>
				{
					if *height > 0.0
					{
						*height = utils::max(0.0, *height - 3. * TILE_SIZE * game_state::DT);
						if *height == 0.0 && *support != 0
						{
							let tile_x = tile_idx % self.width as usize;
							let tile_y = tile_idx / self.width as usize;
							let tile_pos =
								Point2::new(tile_x as f32 * TILE_SIZE, tile_y as f32 * TILE_SIZE);
							state.sfx.play_positional_sound(
								"data/collapse_2.ogg",
								tile_pos,
								camera_pos,
								1.,
							)?;
						}
					}
					if *health < 0.0
					{
						Some(TileKind::Empty)
					}
					else
					{
						None
					}
				}
				TileKind::Empty
				| TileKind::Torch
				| TileKind::Support
				| TileKind::Border
				| TileKind::Grinder
				| TileKind::Jaunter => None,
			};
			if let Some(new_tile) = new_tile
			{
				*tile = new_tile;
			}
		}
		Ok(kill_pos)
	}

	pub fn draw(
		&self, sprite: &str, pos: Point2<f32>, batch: &mut draw_batch::DrawBatch,
		state: &game_state::GameState, lit: bool,
	) -> Result<()>
	{
		let sprite = state.get_sprite(sprite)?;
		for y in 0..self.height
		{
			for x in 0..self.width
			{
				let tile_kind = self.tiles[y as usize * self.width as usize + x as usize];
				let (frame, tile_height) = tile_kind.get_frame_and_height(state.hs.time());
				let (atlas_bmp, offt) = sprite.get_frame("Default", frame);

				// HACK: I don't like this...
				if tile_height != 0.
				{
					let tile_pos = Vector2::new(x as f32 * TILE_SIZE, y as f32 * TILE_SIZE);

					let (atlas_bmp, offt) = sprite.get_frame("Default", 0);
					let pos = utils::round_point(pos + tile_pos) + offt;
					batch.add_bitmap(
						Point2::new(pos.x, pos.y),
						atlas_bmp,
						if lit
						{
							game_state::MaterialKind::Lit
						}
						else
						{
							game_state::MaterialKind::Default
						},
					);
				}
				let tile_pos =
					Vector2::new(x as f32 * TILE_SIZE, y as f32 * TILE_SIZE - tile_height);
				let pos = utils::round_point(pos + tile_pos) + offt;
				batch.add_bitmap(
					Point2::new(pos.x, pos.y),
					atlas_bmp,
					if lit
					{
						game_state::MaterialKind::Lit
					}
					else
					{
						game_state::MaterialKind::Default
					},
				);
			}
		}
		Ok(())
	}

	pub fn draw_support(&self, pos: Point2<f32>, state: &game_state::GameState) -> Result<()>
	{
		state.hs.core.hold_bitmap_drawing(true);
		for y in 0..self.height
		{
			for x in 0..self.width
			{
				let tile_kind = self.tiles[y as usize * self.width as usize + x as usize];

				let tile_pos = Vector2::new(x as f32 * TILE_SIZE, y as f32 * TILE_SIZE);
				let pos = utils::round_point(pos + tile_pos);
				if let TileKind::Rock { support, .. } = tile_kind
				{
					let color = if support > 0
					{
						Color::from_rgb_f(1., 1., 1.)
					}
					else
					{
						Color::from_rgb_f(1., 0.5, 0.)
					};
					state.hs.core.draw_text(
						state.hs.ui_font(),
						color,
						pos.x,
						pos.y,
						FontAlign::Centre,
						&format!("{}", support),
					);
				}
			}
		}
		state.hs.core.hold_bitmap_drawing(false);
		Ok(())
	}
}
