use crate::game_state;

use allegro::*;
use nalgebra::{Point2, UnitQuaternion, Vector2};
use rand::prelude::*;
use serde_derive::{Deserialize, Serialize};
use slhack::sprite;

mod serialize_color
{
	use allegro::*;
	use serde::de::{Deserialize, Deserializer};
	use serde::ser::{Serialize, Serializer};

	pub fn serialize<S>(color: &Color, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		color.to_rgba_array_f().serialize(serializer)
	}

	pub fn deserialize<'de, D>(deserializer: D) -> Result<Color, D::Error>
	where
		D: Deserializer<'de>,
	{
		let color = <[f32; 4]>::deserialize(deserializer)?;
		Ok(Color::from_rgba_f(color[0], color[1], color[2], color[3]))
	}
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Light
{
	#[serde(with = "serialize_color")]
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

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum SolidKind
{
	Player,
	Demon,
	Enemy,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Gravity;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
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

	pub fn get_amount(&self) -> i32
	{
		match self
		{
			DemonKind::Demon1 => 50,
			DemonKind::Demon2 => 100,
			DemonKind::Demon3 => 200,
		}
	}

	pub fn mate_with(&self, _other: DemonKind) -> DemonKind
	{
		*self
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Demon
{
	pub kind: DemonKind,
	pub picked_up: bool,
}

impl Demon
{
	pub fn new(kind: DemonKind, picked_up: bool) -> Self
	{
		Self {
			kind: kind,
			picked_up: picked_up,
		}
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AIState
{
	Idle,
	Move
	{
		dir: f32,
	},
	Jump
	{
		dir: f32,
	},
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AI
{
	pub time_to_decide: f64,
	pub state: AIState,
	pub enemy: bool,
}

impl AI
{
	pub fn new() -> Self
	{
		Self {
			time_to_decide: 0.,
			state: AIState::Idle,
			enemy: false,
		}
	}

	pub fn new_enemy() -> Self
	{
		Self {
			time_to_decide: 0.,
			state: AIState::Idle,
			enemy: true,
		}
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DieAfterAnimationDone;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Explodes;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Langolier
{
	pub time_to_bite: f64,
}

impl Langolier
{
	pub fn new() -> Self
	{
		Self { time_to_bite: 0. }
	}
}

macro_rules! serialize_components {
	($id_enum:ident $hecs_context:ident { $($component:ident ),* $(,)? } ) => {

		#[derive(Serialize, Deserialize)]
		pub enum $id_enum
		{
			$($component,)*
		}

		pub struct $hecs_context;

		impl hecs::serialize::row::SerializeContext for $hecs_context {
		    fn serialize_entity<S>(
		        &mut self,
		        entity: hecs::EntityRef<'_>,
		        mut map: S,
		    ) -> Result<S::Ok, S::Error>
		    where
		        S: serde::ser::SerializeMap,
		    {
				$(
			        hecs::serialize::row::try_serialize::<$component, _, _>(&entity, &$id_enum::$component, &mut map)?;
				)*
		        map.end()
		    }
		}

		impl hecs::serialize::row::DeserializeContext for $hecs_context {
			fn deserialize_entity<'de, M>(
    		    &mut self,
    		    mut map: M,
    		    entity: &mut hecs::EntityBuilder,
    		) -> Result<(), M::Error>
			    where
			        M: serde::de::MapAccess<'de>,
		    {
		        while let Some(key) = map.next_key()? {
		            match key {
						$(
							$id_enum::$component => {
								entity.add::<$component>(map.next_value()?);
							}
						)*
		            }
		        }
		        Ok(())
		    }
		}
	}
}

serialize_components! {
	ComponentId HecsContext
	{
		Light,
		Position,
		Velocity,
		Acceleration,
		Solid,
		Appearance,
		Gravity,
		DemonHolder,
		Demon,
		Drill,
		Mover,
		AI,
		DieAfterAnimationDone,
		Health,
		Climber,
		Explodes,
		Langolier,
	}
}
