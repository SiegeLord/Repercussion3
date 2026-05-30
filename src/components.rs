use crate::game_state;

use allegro::*;
use nalgebra::{Point2, UnitQuaternion, Vector2};
use rand::prelude::*;
use slhack::sprite;

#[derive(Debug, Copy, Clone)]
pub struct Light
{
	pub color: Color,
	pub intensity: f32,
	pub static_: bool,
}

#[derive(Debug, Copy, Clone)]
pub struct Position
{
	pub pos: Point2<f32>,
	old_pos: Point2<f32>,
}

impl Position
{
	pub fn new(pos: Point2<f32>) -> Self
	{
		Self {
			pos: pos,
			old_pos: pos,
		}
	}

	pub fn set_pos(&mut self, pos: Point2<f32>) -> &mut Self
	{
		self.pos = pos;
		self.old_pos = pos;
		self
	}

	pub fn snapshot(&mut self)
	{
		self.old_pos = self.pos;
	}

	pub fn draw_pos(&self, alpha: f32) -> Point2<f32>
	{
		self.pos + alpha * (self.pos - self.old_pos)
	}
}

#[derive(Debug, Copy, Clone)]
pub struct Velocity
{
	pub pos: Vector2<f32>,
}

impl Velocity
{
	pub fn new() -> Self
	{
		Self {
			pos: Vector2::zeros(),
		}
	}
}

#[derive(Debug, Copy, Clone)]
pub struct Acceleration
{
	pub pos: Vector2<f32>,
}

impl Acceleration
{
	pub fn new() -> Self
	{
		Self {
			pos: Vector2::zeros(),
		}
	}
}

#[derive(Debug, Copy, Clone)]
pub struct Solid
{
	pub size: f32,
	pub on_ground: bool,
	pub last_on_ground: f64,
}

impl Solid
{
	pub fn new(size: f32) -> Self
	{
		Self {
			size: size,
			on_ground: false,
			last_on_ground: 0.,
		}
	}
}

#[derive(Debug, Clone)]
pub struct Appearance
{
	pub sprite: String,
	pub animation_state: sprite::AnimationState,
	pub material: game_state::MaterialKind,
	pub speed: f32,
}

impl Appearance
{
	pub fn new(sprite: impl Into<String>) -> Self
	{
		Self {
			sprite: sprite.into(),
			animation_state: sprite::AnimationState::new("Default"),
			speed: 1.,
			material: game_state::MaterialKind::Default,
		}
	}
}

#[derive(Debug, Clone)]
pub struct Gravity;
