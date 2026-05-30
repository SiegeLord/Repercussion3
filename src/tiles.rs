use crate::error::Result;
use crate::{draw_batch, game_state};
use nalgebra::{Point2, Point3, Vector2};
use slhack::utils;

use std::collections::HashMap;
use std::path::Path;

pub const TILE_SIZE: f32 = 32.;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TileKind
{
	Empty,
	Rock,
}

impl TileKind
{
	fn from_id(id: i32) -> Self
	{
		match id
		{
			0 => TileKind::Empty,
			_ => TileKind::Rock,
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
		let mut tiles = vec![TileKind::Rock; (width * height) as usize];

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
		&self, pos: Point2<f32>, batch: &mut draw_batch::DrawBatch, state: &game_state::GameState,
		lit: bool,
	) -> Result<()>
	{
		let sprite = state.get_sprite("data/tiles.cfg")?;
		for y in 0..self.height
		{
			for x in 0..self.width
			{
				let tile_kind = self.tiles[y as usize * self.width as usize + x as usize];
				if tile_kind == TileKind::Empty
				{
					continue;
				}
				let (atlas_bmp, offt) = sprite.get_frame("Default", tile_kind as i32);

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

	pub fn get_tile_kind(&self, pos: Point2<f32>) -> TileKind
	{
		let tile_x = ((pos.x) / TILE_SIZE).floor() as i32;
		let tile_y = ((pos.y) / TILE_SIZE).floor() as i32;
		if tile_x < 0 || tile_x >= self.width || tile_y < 0 || tile_y >= self.height
		{
			return TileKind::Empty;
		}
		self.tiles[tile_y as usize * self.width as usize + tile_x as usize]
	}

	pub fn tile_is_solid(&self, pos: Point2<f32>) -> bool
	{
		self.get_tile_kind(pos) == TileKind::Rock
	}

	/// size is radius.
	pub fn get_escape_dir(
		&self, pos: Point2<f32>, size: f32, avoid_kind: TileKind,
	) -> Option<Vector2<f32>>
	{
		let tile_x = ((pos.x) / TILE_SIZE) as i32;
		let tile_y = ((pos.y) / TILE_SIZE) as i32;

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
				if tile != avoid_kind
				{
					continue;
				}

				let cx = map_x as f32 * TILE_SIZE;
				let cy = map_y as f32 * TILE_SIZE;

				// TODO: This order might be wrong
				let vs = [
					Point2::new(cx, cy),
					Point2::new(cx, cy + TILE_SIZE),
					Point2::new(cx + TILE_SIZE, cy + TILE_SIZE),
					Point2::new(cx + TILE_SIZE, cy),
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
}
