use crate::error::Result;
use crate::{draw_batch, game_state};
use nalgebra::{Point2, Point3, Vector2};
use slhack::utils;

use std::collections::HashMap;
use std::path::Path;

pub const TILE_SIZE: f32 = 32.;
pub const TILE_MAX_HEALTH: f32 = 100.;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TileKind
{
	Empty,
	Rock
	{
		health: f32,
	},
	Torch,
	Support,
}

impl TileKind
{
	fn get_idx(&self) -> i32
	{
		match self
		{
			TileKind::Empty => 0,
			TileKind::Rock { health } =>
			{
				let num_tiles: i32 = 4;
				let f = health / TILE_MAX_HEALTH;
				1 + utils::clamp(
					num_tiles - 1 - (f * num_tiles as f32 - 0.5).trunc() as i32,
					0,
					num_tiles - 1,
				)
			}
			TileKind::Torch => 5,
			TileKind::Support => 6,
		}
	}

	pub fn is_solid(&self) -> bool
	{
		match self
		{
			TileKind::Empty | TileKind::Torch | TileKind::Support => false,
			TileKind::Rock { .. } => true,
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
				health: TILE_MAX_HEALTH
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

		Ok(Self {
			tiles: tiles,
			width: width,
			height: height,
		})
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
				let (atlas_bmp, offt) = sprite.get_frame("Default", tile_kind.get_idx());

				let tile_pos = Vector2::new(x as f32 * TILE_SIZE, y as f32 * TILE_SIZE);
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

	pub fn logic(&mut self)
	{
		for tile in &mut self.tiles
		{
			let new_tile = match tile
			{
				TileKind::Rock { health } =>
				{
					if *health < 0.
					{
						Some(TileKind::Empty)
					}
					else
					{
						None
					}
				}
				TileKind::Empty => None,
				TileKind::Torch => None,
				TileKind::Support => None,
			};
			if let Some(new_tile) = new_tile
			{
				*tile = new_tile;
			}
		}
	}
}
