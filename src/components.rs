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

	pub fn with_pos(mut self, pos: Vector2<f32>) -> Self
	{
		self.pos = pos;
		self
	}
}

#[derive(Debug, Copy, Clone)]
pub struct Acceleration
{
	pub pos: Vector2<f32>,
	pub last_change: Vector2<f32>,
}

impl Acceleration
{
	pub fn new() -> Self
	{
		Self {
			pos: Vector2::zeros(),
			last_change: Vector2::zeros(),
		}
	}
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SolidKind
{
	Player,
	Demon,
}

#[derive(Debug, Copy, Clone)]
pub struct Solid
{
	pub size: f32,
	pub on_ground: bool,
	pub last_on_ground: f64,
	pub kind: SolidKind,
}

impl Solid
{
	pub fn new(size: f32, kind: SolidKind) -> Self
	{
		Self {
			size: size,
			on_ground: false,
			last_on_ground: 0.,
			kind: kind,
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
	pub animated: bool,
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
			animated: true,
		}
	}

	pub fn with_animated(mut self, animated: bool) -> Self
	{
		self.animated = animated;
		self
	}
}

#[derive(Debug, Copy, Clone)]
pub struct Gravity;

#[derive(Debug, Clone)]
pub struct DemonHolder
{
	pub demon: Option<hecs::Entity>,
}

impl DemonHolder
{
	pub fn new() -> Self
	{
		Self { demon: None }
	}
}

#[derive(Debug, Copy, Clone)]
pub enum DemonKind
{
	Demon1,
}

#[derive(Debug, Clone)]
pub struct Drill
{
	pub want_left: bool,
	pub want_right: bool,
	pub want_up: bool,
	pub want_down: bool,
}

impl Drill
{
	pub fn new() -> Self
	{
		Self {
			want_left: false,
			want_right: false,
			want_up: false,
			want_down: false,
		}
	}
}
