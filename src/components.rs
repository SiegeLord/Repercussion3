use crate::game_state;

use allegro::*;
use nalgebra::{Point2, UnitQuaternion, Vector2};
use rand::prelude::*;
use slhack::sprite;

#[derive(Debug, Copy, Clone)]
pub struct Light
{
	pub color: Color,
	pub y_offt: f32,
}

impl Light
{
	pub fn new(color: Color, y_offt: f32) -> Self
	{
		Self {
			color: color,
			y_offt: y_offt,
		}
	}
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
	pub want_pickup: bool,
	pub want_eat: bool,
}

impl DemonHolder
{
	pub fn new() -> Self
	{
		Self {
			demon: None,
			want_pickup: false,
			want_eat: false,
		}
	}
}

#[derive(Debug, Copy, Clone)]
pub enum DemonKind
{
	Demon1,
	Demon2,
	Demon3,
}

impl DemonKind
{
	pub fn get_color(&self) -> Color
	{
		match self
		{
			DemonKind::Demon1 => Color::from_rgb_f(1., 1., 0.),
			DemonKind::Demon2 => Color::from_rgb_f(0., 1., 1.),
			DemonKind::Demon3 => Color::from_rgb_f(1., 0., 1.),
		}
	}

	pub fn mate_with(&self, _other: DemonKind) -> DemonKind
	{
		*self
	}
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

#[derive(Debug, Clone)]
pub struct Mover
{
	pub want_move_left: f32,
	pub want_move_right: f32,
	pub want_move_up: f32,
	pub want_move_down: f32,
	pub want_jump: bool,
}

impl Mover
{
	pub fn new() -> Mover
	{
		Self {
			want_move_left: 0.0,
			want_move_right: 0.0,
			want_move_down: 0.0,
			want_move_up: 0.0,
			want_jump: false,
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub enum AIState
{
	Idle,
	Jump
	{
		dir: f32,
	},
}

#[derive(Debug, Clone)]
pub struct AI
{
	pub time_to_decide: f64,
	pub state: AIState,
}

impl AI
{
	pub fn new() -> Self
	{
		Self {
			time_to_decide: 0.,
			state: AIState::Idle,
		}
	}
}

#[derive(Debug, Clone)]
pub struct DieAfterAnimationDone;

#[derive(Debug, Clone)]
pub struct Health
{
	pub cur_health: f32,
	pub max_health: f32,
}

impl Health
{
	pub fn new(max_health: f32) -> Self
	{
		Self {
			cur_health: max_health,
			max_health: max_health,
		}
	}
}

#[derive(Debug, Clone)]
pub struct Climber
{
	pub climbing: bool,
}

impl Climber
{
	pub fn new() -> Self
	{
		Self { climbing: false }
	}
}

#[derive(Debug, Clone)]
pub struct Explodes;
