use crate::error::{Result, ResultHelper};
use crate::game_state::DT;
use crate::{components as comps, draw_batch, game_state, tiles, ui, utils};
use allegro::*;
use allegro_audio::*;
use allegro_font::*;
use allegro_primitives::*;
use nalgebra::{Matrix4, Point2, Vector2};
use rand::prelude::*;
use serde_derive::{Deserialize, Serialize};
use slhack::{controls, scene, spatial_grid, sprite, ui as slhack_ui};

use std::collections::HashMap;
use std::f32::consts::PI;

const MAX_SPEED: f32 = 150.0;
const PICKUP_RADIUS: f32 = 16.0;
const DRILL_RADIUS: f32 = 18.0;
const TORCH_COST: i32 = 20;
const SUPPORT_COST: i32 = 10;
const JAUNTER_COST: i32 = 30;

pub struct Game
{
	map: Map,
	subscreens: ui::SubScreens,
}

impl Game
{
	pub fn new(resume: bool, state: &mut game_state::GameState) -> Result<Self>
	{
		state
			.sfx
			.play_music("data/Repercussion3_1.ogg", 1.0, &state.hs.core);
		let mut map = Map::new(state)?;
		if resume
		{
			map.load(state)?;
		}
		Ok(Self {
			map: map,
			subscreens: ui::SubScreens::new(state),
		})
	}

	pub fn logic(
		&mut self, state: &mut game_state::GameState,
	) -> Result<Option<game_state::NextScreen>>
	{
		if self.subscreens.is_empty()
		{
			self.map.logic(state)
		}
		else
		{
			Ok(None)
		}
	}

	pub fn input(
		&mut self, event: &Event, state: &mut game_state::GameState,
	) -> Result<Option<game_state::NextScreen>>
	{
		if self.subscreens.is_empty()
		{
			let mut in_game_menu = false;
			let handled = false; // In case there's other in-game UI to handle this.
			if state
				.hs
				.game_ui_controls
				.get_action_state(slhack_ui::UIAction::Cancel)
				> 0.5
			{
				in_game_menu = true;
			}
			else if !handled
			{
				let res = self.map.input(event, state);
				if let Ok(Some(game_state::NextScreen::InGameMenu)) = res
				{
					in_game_menu = true;
				}
				else
				{
					return res;
				}
			}
			if in_game_menu
			{
				self.subscreens
					.push(ui::SubScreen::InGameMenu(ui::InGameMenu::new(state)));
				self.subscreens.reset_transition(state);
			}
		}
		else
		{
			if let Some(action) = self.subscreens.input(state, event)?
			{
				match action
				{
					ui::Action::MainMenu =>
					{
						self.map.save(state)?;
						return Ok(Some(game_state::NextScreen::Menu));
					}
					_ => (),
				}
			}
			if self.subscreens.is_empty()
			{
				state.controls.clear_action_states();
			}
		}
		Ok(None)
	}

	pub fn draw(&mut self, state: &mut game_state::GameState) -> Result<()>
	{
		if !self.subscreens.is_empty()
		{
			state
				.hs
				.core
				.clear_to_color(Color::from_rgb_f(0.0, 0.0, 0.0));
			self.subscreens.draw(state);
		}
		else
		{
			self.map.draw(state)?;
		}
		Ok(())
	}

	pub fn resize(&mut self, state: &game_state::GameState)
	{
		self.subscreens.resize(state);
	}
}

fn spawn_player(
	pos: Point2<f32>, world: &mut hecs::World, state: &mut game_state::GameState,
) -> Result<hecs::Entity>
{
	let sprite_name = "data/player.cfg";
	state.cache_sprite(sprite_name)?;

	let entity = world.spawn((
		comps::Position::new(pos),
		comps::Acceleration::new(),
		comps::Velocity::new(),
		comps::Appearance::new(sprite_name),
		comps::Solid::new(24., comps::SolidKind::Player),
		comps::Gravity,
		comps::DemonHolder::new(),
		comps::Drill::new(),
		comps::Light::new(Color::from_rgba(0, 0, 0, 6), 0.),
		comps::Mover::new(),
		comps::Health::new(100.),
		comps::Climber::new(),
	));

	Ok(entity)
}

fn spawn_langolier(
	pos: Point2<f32>, world: &mut hecs::World, state: &mut game_state::GameState,
) -> Result<hecs::Entity>
{
	let sprite_name = "data/langolier.cfg";
	state.cache_sprite(sprite_name)?;

	let entity = world.spawn((
		comps::Position::new(pos),
		comps::Acceleration::new(),
		comps::Velocity::new(),
		comps::Appearance::new(sprite_name),
		comps::Solid::new(24., comps::SolidKind::Enemy),
		comps::Gravity,
		comps::Light::new(Color::from_rgba(0, 0, 0, 6), 0.),
		comps::Mover::new(),
		comps::Health::new(100.),
		comps::AI::new_enemy(),
		comps::Langolier::new(),
	));

	Ok(entity)
}

fn spawn_demon(
	kind: comps::DemonKind, pos: Point2<f32>, pos_vel: Vector2<f32>, world: &mut hecs::World,
	state: &mut game_state::GameState,
) -> Result<hecs::Entity>
{
	let sprite_name = "data/demon1.cfg";
	state.cache_sprite(sprite_name)?;

	let entity = world.spawn((
		comps::Position::new(pos),
		comps::Acceleration::new(),
		comps::Velocity::new().with_pos(pos_vel),
		comps::Appearance::new(sprite_name).with_animated(false),
		comps::Solid::new(24., comps::SolidKind::Demon),
		comps::Light::new(kind.get_color(), 0.),
		comps::AI::new(),
		comps::Gravity,
		comps::Mover::new(),
		comps::Health::new(50.),
		comps::Explodes,
		kind,
	));

	Ok(entity)
}

fn spawn_explosion(
	pos: Point2<f32>, world: &mut hecs::World, state: &mut game_state::GameState,
) -> Result<hecs::Entity>
{
	let sprite_name = "data/explosion.cfg";
	state.cache_sprite(sprite_name)?;

	let entity = world.spawn((
		comps::Position::new(pos),
		comps::Appearance::new(sprite_name),
		comps::Light::new(Color::from_rgb_f(1., 1., 1.), 0.),
		comps::DieAfterAnimationDone,
	));

	Ok(entity)
}

fn spawn_demon_item(
	kind: comps::DemonKind, pos: Point2<f32>, world: &mut hecs::World,
	state: &mut game_state::GameState,
) -> Result<hecs::Entity>
{
	let sprite_name = "data/demon1.cfg";
	state.cache_sprite(sprite_name)?;

	let entity = world.spawn((
		comps::Position::new(pos),
		comps::Appearance::new(sprite_name).with_animated(false),
		comps::Light::new(kind.get_color(), 0.),
		kind,
	));

	Ok(entity)
}

fn dir_name(vx: f32) -> &'static str
{
	if vx > 0. { "Right" } else { "Left" }
}

#[derive(Debug, Copy, Clone)]
struct GridInner
{
	id: hecs::Entity,
	pos: Point2<f32>,
	solid: comps::Solid,
}

mod world_serialize
{
	use serde::de::{Deserialize, Deserializer};
	use serde::ser::{Serialize, Serializer};

	pub fn serialize<S>(world: &hecs::World, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		hecs::serialize::row::serialize(world, &mut crate::components::HecsContext, serializer)
	}

	pub fn deserialize<'de, D>(deserializer: D) -> Result<hecs::World, D::Error>
	where
		D: Deserializer<'de>,
	{
		hecs::serialize::row::deserialize(&mut crate::components::HecsContext, deserializer)
	}
}

#[derive(Serialize, Deserialize)]
struct Save
{
	tick: i64,
	money: i32,
	player: hecs::Entity,
	tiles: tiles::Tiles,
	camera_pos: comps::Position,
	#[serde(with = "world_serialize")]
	world: hecs::World,
}

struct Map
{
	world: hecs::World,
	tiles: tiles::Tiles,
	camera_pos: comps::Position,
	player: hecs::Entity,
	money: i32,
	money_change_amount: i32,
	money_change_time: f64,
	health_change_amount: i32,
	health_change_time: f64,
	drill_sound: SampleInstance,
}

impl Map
{
	fn new(state: &mut game_state::GameState) -> Result<Self>
	{
		let mut world = hecs::World::new();
		state.cache_sprite("data/tiles.cfg")?;
		state.cache_sprite("data/shadow_tiles.cfg")?;
		state.cache_bitmap("data/circle.png")?;

		let pos = Point2::new(tiles::TILE_SIZE * 10., tiles::TILE_SIZE * 10.);
		let player = spawn_player(pos, &mut world, state)?;

		let pos = Point2::new(tiles::TILE_SIZE * 12., tiles::TILE_SIZE * 10.);
		spawn_demon(
			comps::DemonKind::Demon1,
			pos,
			Vector2::zeros(),
			&mut world,
			state,
		)?;

		let pos = Point2::new(tiles::TILE_SIZE * 8., tiles::TILE_SIZE * 10.);
		spawn_demon(
			comps::DemonKind::Demon2,
			pos,
			Vector2::zeros(),
			&mut world,
			state,
		)?;

		let pos = Point2::new(tiles::TILE_SIZE * 14., tiles::TILE_SIZE * 10.);
		spawn_demon(
			comps::DemonKind::Demon3,
			pos,
			Vector2::zeros(),
			&mut world,
			state,
		)?;

		let pos = Point2::new(tiles::TILE_SIZE * 4., tiles::TILE_SIZE * 10.);
		spawn_langolier(pos, &mut world, state)?;
		spawn_langolier(pos, &mut world, state)?;

		let drill_sound = state.sfx.play_continuous_sound("data/drill.ogg", 1.)?;
		drill_sound.set_gain(0.0).ok();

		Ok(Self {
			world: world,
			tiles: tiles::Tiles::new(16, 16)?,
			camera_pos: comps::Position::new(Point2::origin()),
			player: player,
			money_change_time: 0.,
			money_change_amount: 0,
			health_change_time: 0.,
			health_change_amount: 0,
			money: 100,
			drill_sound: drill_sound,
		})
	}

	fn save(&mut self, state: &game_state::GameState) -> Result<()>
	{
		let mut dummy_world = hecs::World::new();
		std::mem::swap(&mut dummy_world, &mut self.world);
		let mut save = Save {
			tick: state.hs.tick,
			world: dummy_world,
			camera_pos: self.camera_pos,
			money: self.money,
			player: self.player,
			tiles: self.tiles.clone(),
		};
		println!("Saving");
		utils::save_user_data(&state.hs.core, "save.cfg", &save)?;
		std::mem::swap(&mut save.world, &mut self.world);
		Ok(())
	}

	fn load(&mut self, state: &mut game_state::GameState) -> Result<()>
	{
		println!("Loading");
		if let Some(save) = utils::load_user_data::<Save>(&state.hs.core, "save.cfg")?
		{
			state.hs.tick = save.tick;
			self.world = save.world;
			self.camera_pos = save.camera_pos;
			self.money = save.money;
			self.player = save.player;
			self.tiles = save.tiles;

			for (_, appearance) in self.world.query_mut::<&comps::Appearance>()
			{
				state.cache_sprite(&appearance.sprite)?;
			}
		}
		Ok(())
	}

	fn logic(&mut self, state: &mut game_state::GameState)
	-> Result<Option<game_state::NextScreen>>
	{
		if self.world.contains(self.player)
		{
			if state
				.controls
				.get_action_state(game_state::Action::QuickSave)
				> 0.5
			{
				self.save(state)?;
			}
			state
				.controls
				.clear_action_state(game_state::Action::QuickSave);
			if state
				.controls
				.get_action_state(game_state::Action::QuickLoad)
				> 0.5
			{
				self.load(state)?;
			}
			state
				.controls
				.clear_action_state(game_state::Action::QuickLoad);
		}

		let mut to_die = vec![];
		let mut spawn_fns: Vec<
			Box<dyn FnOnce(&mut Map, &mut game_state::GameState) -> Result<hecs::Entity>>,
		> = vec![];
		let mut rng = rand::thread_rng();

		// Position snapshotting.
		for (_, position) in self.world.query::<&mut comps::Position>().iter()
		{
			position.snapshot();
		}
		self.camera_pos.snapshot();

		// Zero out acceleration for the frame.
		// XXX: Acceleration kinda ends up acting as a frame accumulator. It's akin to how we zero
		// out the forces in physics simulators. Is this actually okay?
		for (_, acceleration) in self.world.query_mut::<&mut comps::Acceleration>()
		{
			acceleration.pos = Vector2::zeros();
		}

		// Player input.
		let mut player_pos = None;
		if let Ok((drill, position, mover, demon_holder)) = self.world.query_one_mut::<(
			&mut comps::Drill,
			&comps::Position,
			&mut comps::Mover,
			&mut comps::DemonHolder,
		)>(self.player)
		{
			player_pos = Some(position.pos);
			mover.want_move_left = state
				.controls
				.get_action_state(game_state::Action::MoveLeft);
			mover.want_move_right = state
				.controls
				.get_action_state(game_state::Action::MoveRight);
			mover.want_move_up = state.controls.get_action_state(game_state::Action::MoveUp);
			mover.want_move_down = state
				.controls
				.get_action_state(game_state::Action::MoveDown);
			mover.want_jump = state.controls.get_action_state(game_state::Action::Jump) > 0.5;

			drill.want_left = state
				.controls
				.get_action_state(game_state::Action::DrillLeft)
				> 0.5;
			drill.want_right = state
				.controls
				.get_action_state(game_state::Action::DrillRight)
				> 0.5;
			drill.want_up = state.controls.get_action_state(game_state::Action::DrillUp) > 0.5;
			drill.want_down = state
				.controls
				.get_action_state(game_state::Action::DrillDown)
				> 0.5;

			demon_holder.want_pickup =
				state.controls.get_action_state(game_state::Action::Pickup) > 0.5;
			state
				.controls
				.clear_action_state(game_state::Action::Pickup);
			demon_holder.want_eat = state
				.controls
				.get_action_state(game_state::Action::EatDemon)
				> 0.5;
			state
				.controls
				.clear_action_state(game_state::Action::EatDemon);

			if state
				.controls
				.get_action_state(game_state::Action::PlaceTorch)
				> 0.5
			{
				if self.money >= TORCH_COST
				{
					// XXX: Same question about shift.
					if let Some(tile) = self.tiles.get_tile_kind_mut(
						position.pos + Vector2::new(tiles::TILE_SIZE / 2., tiles::TILE_SIZE / 2.),
					)
					{
						if *tile == tiles::TileKind::Empty
						{
							state.sfx.play_positional_sound(
								"data/build.ogg",
								position.pos,
								self.camera_pos.pos,
								1.,
							)?;
							self.money -= TORCH_COST;
							self.money_change_amount = -TORCH_COST;
							self.money_change_time = state.hs.time();
							*tile = tiles::TileKind::Torch;
						}
					}
				}
			}
			state
				.controls
				.clear_action_state(game_state::Action::PlaceTorch);

			if state
				.controls
				.get_action_state(game_state::Action::PlaceSupport)
				> 0.5
			{
				if self.money >= SUPPORT_COST
				{
					// XXX: Same question about shift.
					if let Some(tile) = self.tiles.get_tile_kind_mut(
						position.pos + Vector2::new(tiles::TILE_SIZE / 2., tiles::TILE_SIZE / 2.),
					)
					{
						if *tile == tiles::TileKind::Empty || *tile == tiles::TileKind::Torch
						{
							state.sfx.play_positional_sound(
								"data/build.ogg",
								position.pos,
								self.camera_pos.pos,
								1.,
							)?;
							self.money -= SUPPORT_COST;
							self.money_change_amount = -SUPPORT_COST;
							self.money_change_time = state.hs.time();
							*tile = tiles::TileKind::Support;
						}
					}
				}
			}
			state
				.controls
				.clear_action_state(game_state::Action::PlaceSupport);

			if state
				.controls
				.get_action_state(game_state::Action::PlaceJaunter)
				> 0.5
			{
				if self.money >= JAUNTER_COST
				{
					// XXX: Same question about shift.
					if let Some(tile) = self.tiles.get_tile_kind_mut(
						position.pos + Vector2::new(tiles::TILE_SIZE / 2., tiles::TILE_SIZE / 2.),
					)
					{
						if *tile == tiles::TileKind::Empty
						{
							state.sfx.play_positional_sound(
								"data/build.ogg",
								position.pos,
								self.camera_pos.pos,
								1.,
							)?;
							self.money -= JAUNTER_COST;
							self.money_change_amount = -JAUNTER_COST;
							self.money_change_time = state.hs.time();
							*tile = tiles::TileKind::Jaunter;
						}
					}
				}
			}
			state
				.controls
				.clear_action_state(game_state::Action::PlaceJaunter);
		}

		// AI.
		for (_id, (ai, position, mover)) in self
			.world
			.query::<(&mut comps::AI, &comps::Position, &mut comps::Mover)>()
			.iter()
		{
			let next_state_and_duration = if state.hs.time() > ai.time_to_decide
			{
				let next_states_and_weights = if ai.enemy
				{
					let mut dir = rng.gen_range(-1.0..1.0);
					let mut pursue_weight = 3.0;
					if let Some(player_pos) = player_pos
					{
						let pos = position.pos;
						if (player_pos - pos).norm() < tiles::TILE_SIZE * 6.0
						{
							dir = (player_pos.x - pos.x).signum();
							pursue_weight = 100.;
						}
					}

					[
						(comps::AIState::Idle, rng.gen_range(1.0..2.0), 10.0),
						(comps::AIState::Jump { dir: dir }, 1., pursue_weight),
						(comps::AIState::Move { dir: dir }, 1., 2. * pursue_weight),
					]
				}
				else
				{
					[
						(comps::AIState::Idle, rng.gen_range(1.0..2.0), 10.0),
						(
							comps::AIState::Jump {
								dir: rng.gen_range(-1.0..1.0),
							},
							0.5,
							3.0,
						),
						(
							comps::AIState::Move {
								dir: rng.gen_range(-1.0..1.0),
							},
							0.5,
							2.0,
						),
					]
				};
				next_states_and_weights
					.choose_weighted(&mut rng, |(_, _, weight)| *weight)
					.ok()
					.cloned()
			}
			else
			{
				None
			};

			if let Some((next_state, duration, _)) = next_state_and_duration
			{
				match next_state
				{
					comps::AIState::Move { .. } | comps::AIState::Jump { .. } =>
					{
						if ai.enemy
						{
							state.sfx.play_positional_sound(
								"data/click.ogg",
								position.pos,
								self.camera_pos.pos,
								1.,
							)?;
						}
					}
					_ => (),
				}
				ai.state = next_state;
				ai.time_to_decide = state.hs.time() + duration;
			}

			match ai.state
			{
				comps::AIState::Idle =>
				{
					mover.want_jump = false;
					mover.want_move_left = 0.;
					mover.want_move_right = 0.;
				}
				comps::AIState::Jump { dir } =>
				{
					mover.want_jump = true;
					mover.want_move_left = -dir.min(0.);
					mover.want_move_right = dir.max(0.);
				}
				comps::AIState::Move { dir } =>
				{
					mover.want_jump = false;
					mover.want_move_left = -dir.min(0.);
					mover.want_move_right = dir.max(0.);
				}
			}
		}

		// Climber.
		for (_, (position, climber)) in self
			.world
			.query_mut::<(&comps::Position, &mut comps::Climber)>()
		{
			// XXX: Same question about shift
			climber.climbing = tiles::TileKind::Support
				== self.tiles.get_tile_kind(
					position.pos + Vector2::new(tiles::TILE_SIZE / 2., tiles::TILE_SIZE / 2.),
				);
		}

		// Mover.
		for (id, (position, velocity, acceleration, solid, mover)) in self
			.world
			.query::<(
				&mut comps::Position,
				&mut comps::Velocity,
				&mut comps::Acceleration,
				&mut comps::Solid,
				&comps::Mover,
			)>()
			.iter()
		{
			let mut climber = self.world.get::<&mut comps::Climber>(id).ok();
			let climbing = climber
				.as_mut()
				.map(|climber| climber.climbing)
				.unwrap_or(false);
			let right_left = mover.want_move_right - mover.want_move_left;
			let down_up = mover.want_move_down - mover.want_move_up;

			let want_drill = self
				.world
				.get::<&comps::Drill>(id)
				.map(|drill| {
					drill.want_left || drill.want_right || drill.want_down || drill.want_up
				})
				.unwrap_or(false);

			if id == self.player
			{
				if want_drill
				{
					self.drill_sound.set_gain(1.0).ok();
				}
				else
				{
					self.drill_sound.set_gain(0.).ok();
				}
			}

			let control = if solid.on_ground { 1. } else { 0.5 };
			let mut can_move = !want_drill;

			// Jaunting.
			//
			// XXX: Same question about shift.
			if mover.want_jump && solid.on_ground
			{
				if id == self.player
				{
					state.sfx.play_positional_sound(
						"data/jump.ogg",
						position.pos,
						self.camera_pos.pos,
						1.,
					)?;
				}

				if let Some(jaunt_pos) = self.tiles.get_jaunt_dest(
					position.pos + Vector2::new(tiles::TILE_SIZE / 2., tiles::TILE_SIZE / 2.),
				)
				{
					state.sfx.play_positional_sound(
						"data/jaunt.ogg",
						position.pos,
						self.camera_pos.pos,
						1.,
					)?;
					state.sfx.play_positional_sound(
						"data/jaunt.ogg",
						jaunt_pos,
						self.camera_pos.pos,
						1.,
					)?;

					position.set_pos(jaunt_pos);
					can_move = false;
					solid.last_on_ground = -100.;
				}
			}

			if can_move
			{
				if climbing
				{
					velocity.pos = 64. * Vector2::new(right_left, down_up);
				}
				else
				{
					acceleration.pos.x = 256. * right_left * control;
					if right_left.abs() > 1e-1
					{
						acceleration.last_change = acceleration.pos;
					}
					if mover.want_jump && (state.hs.time() - solid.last_on_ground) < 0.2
					{
						velocity.pos.y -= 64.;
						//println!("Jump: {}", velocity.pos.y);
					}
				}
			}
		}

		// Friction.
		for (id, (velocity, acceleration, solid)) in self
			.world
			.query::<(
				&mut comps::Velocity,
				&mut comps::Acceleration,
				&comps::Solid,
			)>()
			.iter()
		{
			let climbing = self
				.world
				.get::<&comps::Climber>(id)
				.map(|climber| climber.climbing)
				.unwrap_or(false);
			if solid.on_ground && acceleration.pos.x.abs() == 0. && !climbing
			{
				let decel = 2048.;
				if velocity.pos.x.abs() > 0. && acceleration.pos.x == 0.
				{
					if velocity.pos.x.abs() <= decel * DT
					{
						velocity.pos.x = 0.;
					}
					else
					{
						acceleration.pos.x = -velocity.pos.x.signum() * decel;
					}
				}
			}
		}

		// Gravity.
		for (id, acceleration) in self.world.query::<&mut comps::Acceleration>().iter()
		{
			let climbing = self
				.world
				.get::<&comps::Climber>(id)
				.map(|climber| climber.climbing)
				.unwrap_or(false);
			if !climbing
			{
				acceleration.pos.y = 512.;
			}
		}

		// Velocity
		for (_, (velocity, acceleration)) in self
			.world
			.query::<(&mut comps::Velocity, &mut comps::Acceleration)>()
			.iter()
		{
			velocity.pos += acceleration.pos * DT;
			if velocity.pos.x.abs() > MAX_SPEED
			{
				velocity.pos.x = velocity.pos.x * MAX_SPEED / velocity.pos.x.abs();
			}
			if velocity.pos.y.abs() > MAX_SPEED
			{
				velocity.pos.y = velocity.pos.y * MAX_SPEED / velocity.pos.y.abs();
			}
		}

		// Position.
		for (_, (position, velocity)) in self
			.world
			.query::<(&mut comps::Position, &comps::Velocity)>()
			.iter()
		{
			position.pos += velocity.pos * DT;
		}

		// Solid.
		for (id, (position, velocity, solid)) in self
			.world
			.query::<(
				&mut comps::Position,
				&mut comps::Velocity,
				&mut comps::Solid,
			)>()
			.iter()
		{
			if let Some(escape_dir) = self.tiles.get_escape_dir(
				position.pos + Vector2::new(solid.size, solid.size) / 2.,
				solid.size / 2.,
				|tile_kind| tile_kind.is_solid(),
			)
			{
				position.pos += escape_dir;
				let old_on_ground = solid.on_ground;
				solid.on_ground = escape_dir.y < -1e-3;
				if solid.on_ground
				{
					if !old_on_ground
						&& id == self.player
						&& (state.hs.time - solid.last_on_ground) > 0.1
					{
						state.sfx.play_positional_sound(
							"data/land.ogg",
							position.pos,
							self.camera_pos.pos,
							1.,
						)?;
					}
					solid.last_on_ground = state.hs.time();
				}
				let norm_escape_dir = escape_dir.normalize();
				if velocity.pos.norm() > 0.
				{
					let proj_velocity = norm_escape_dir * velocity.pos.dot(&norm_escape_dir);
					//println!("id: {:?} ed: {:?} v: {:?}, pv: {:?}", _id, escape_dir, velocity, proj_velocity);
					velocity.pos -= proj_velocity;
				}
			}
			else
			{
				solid.on_ground = false;
			}
		}

		// Position hack.
		for (_, (position, velocity)) in self
			.world
			.query::<(&mut comps::Position, &comps::Velocity)>()
			.iter()
		{
			// HACK! to remove jitter
			if velocity.pos.norm() < 1e-1
			{
				position.pos = utils::round_point(position.pos);
			}
		}

		// Collision detection
		let mut grid = spatial_grid::SpatialGrid::new(
			self.tiles.width as usize,
			self.tiles.height as usize,
			tiles::TILE_SIZE,
			tiles::TILE_SIZE,
		);

		for (id, (position, solid)) in self.world.query_mut::<(&comps::Position, &comps::Solid)>()
		{
			let margin = 8.;
			let r = solid.size + margin;
			let x = position.pos.x;
			let y = position.pos.y;
			grid.push(spatial_grid::entry(
				Point2::new(x - r, y - r),
				Point2::new(x + r, y + r),
				GridInner {
					pos: position.pos,
					id: id,
					solid: *solid,
				},
			));
		}

		// Demon holder.
		if self.world.contains(self.player)
		{
			let r = PICKUP_RADIUS;
			let mut do_spawn_demon = None;
			let mut pickup_demon = None;
			if let Ok((position, velocity, acceleration, demon_holder, health)) =
				self.world.query_one_mut::<(
					&mut comps::Position,
					&comps::Velocity,
					&comps::Acceleration,
					&mut comps::DemonHolder,
					&mut comps::Health,
				)>(self.player)
			{
				if demon_holder.want_pickup
				{
					let diff = Vector2::new(r, r);
					let pos = position.pos;
					let entries = grid.query_rect(pos - diff, pos + diff, |other| {
						let other_id = other.inner.id;
						if self.player == other_id
						{
							false
						}
						else if other.inner.solid.kind != comps::SolidKind::Demon
						{
							false
						}
						else
						{
							(other.inner.pos - position.pos).norm() < r
						}
					});

					let mut play_sound = false;
					if let Some(demon_entry) = entries.iter().copied().next()
					{
						let item_pos = position.pos
							+ Vector2::new(
								PICKUP_RADIUS * acceleration.last_change.x.signum(),
								-8.,
							);
						pickup_demon = Some((demon_entry.inner.id, item_pos));
						play_sound = true;
					}
					if let Some(demon_item_id) = demon_holder.demon.take()
					{
						do_spawn_demon = Some((
							demon_item_id,
							position.pos,
							velocity.pos,
							acceleration.last_change.x.signum(),
						));
						play_sound = true;
					}
					if play_sound
					{
						state.sfx.play_positional_sound(
							"data/pickup.ogg",
							position.pos,
							self.camera_pos.pos,
							1.,
						)?;
					}
				}
				if demon_holder.want_eat
				{
					if let Some(demon_item_id) = demon_holder.demon.take()
					{
						state.sfx.play_positional_sound(
							"data/eat.ogg",
							position.pos,
							self.camera_pos.pos,
							1.,
						)?;

						to_die.push(demon_item_id);
						let old_health = health.cur_health;
						health.cur_health =
							utils::clamp(health.cur_health + 20.0, 0.0, health.max_health);
						self.health_change_amount = (health.cur_health - old_health) as i32;
						self.health_change_time = state.hs.time();
					}
				}
			}
			if let Some((pickup_demon_id, item_pos)) = pickup_demon
			{
				let demon_kind = *self
					.world
					.get::<&comps::DemonKind>(pickup_demon_id)
					.unwrap();
				let item = spawn_demon_item(demon_kind, item_pos, &mut self.world, state)?;
				let mut demon_holder = self
					.world
					.get::<&mut comps::DemonHolder>(self.player)
					.unwrap();
				demon_holder.demon = Some(item);
				to_die.push(pickup_demon_id);
			}
			if let Some((demon_item_id, pos, pos_vel, sign)) = do_spawn_demon
			{
				let demon_kind = *self.world.get::<&comps::DemonKind>(demon_item_id).unwrap();
				spawn_demon(
					demon_kind,
					pos + Vector2::new(r * sign, -8.),
					pos_vel + Vector2::new(64. * sign, -64.),
					&mut self.world,
					state,
				)?;
				to_die.push(demon_item_id);
			}
		}

		// Holder.
		let mut item_positions = vec![];
		for (_, (position, acceleration, demon_holder)) in self
			.world
			.query::<(&comps::Position, &comps::Acceleration, &comps::DemonHolder)>()
			.iter()
		{
			let item_pos = position.pos
				+ Vector2::new(PICKUP_RADIUS * acceleration.last_change.x.signum(), -8.);
			if let Some(demon_item_id) = demon_holder.demon
			{
				item_positions.push((demon_item_id, item_pos));
			}
		}
		for (demon_item_id, item_pos) in item_positions
		{
			let position = self
				.world
				.query_one_mut::<&mut comps::Position>(demon_item_id)
				.unwrap();
			let need_snapshot = (position.pos - item_pos).norm() > 16.;
			position.pos = item_pos;
			// XXX: BAD
			if need_snapshot
			{
				position.snapshot();
			}
		}

		// Drill.
		for (id, (position, drill)) in self
			.world
			.query::<(&comps::Position, &comps::Drill)>()
			.iter()
		{
			if self
				.world
				.get::<&comps::DemonHolder>(id)
				.map(|demon_holder| demon_holder.demon.is_some())
				.unwrap_or(false)
			{
				continue;
			}
			let drill_dir = if drill.want_left
			{
				Some(Vector2::new(-1., 0.))
			}
			else if drill.want_right
			{
				Some(Vector2::new(1., 0.))
			}
			else if drill.want_up
			{
				Some(Vector2::new(0., -1.))
			}
			else if drill.want_down
			{
				Some(Vector2::new(0., 1.))
			}
			else
			{
				None
			};

			if let Some(drill_dir) = drill_dir
			{
				// XXX: I don't fully understand why the half tile shift is needed, the position
				// should be in the center of the sprite, and the tiles should have (0, 0) as their
				// top left corner.
				if let Some(tile) = self.tiles.get_tile_kind_mut(
					position.pos
						+ DRILL_RADIUS * drill_dir
						+ Vector2::new(tiles::TILE_SIZE / 2., -(32. - 24.) + tiles::TILE_SIZE / 2.),
				)
				{
					match tile
					{
						tiles::TileKind::Rock { health, .. } =>
						{
							*health -= 200. * DT;
						}
						tiles::TileKind::Empty
						| tiles::TileKind::Torch
						| tiles::TileKind::Border
						| tiles::TileKind::Grinder => (),
						tiles::TileKind::Support =>
						{
							if id == self.player
							{
								self.money += SUPPORT_COST;
								self.money_change_amount = SUPPORT_COST;
								self.money_change_time = state.hs.time();
							}
							*tile = tiles::TileKind::Empty;
						}
						tiles::TileKind::Jaunter =>
						{
							*tile = tiles::TileKind::Empty;
						}
					};
				}
			}
		}

		let mut explosions = vec![];

		// Demon breeding.
		for (id, (position, solid, health, demon_kind)) in self
			.world
			.query::<(
				&comps::Position,
				&comps::Solid,
				&mut comps::Health,
				&comps::DemonKind,
			)>()
			.iter()
		{
			if !rng.gen_bool(1e-3)
			{
				continue;
			}
			let r = solid.size;
			let diff = Vector2::new(r, r);
			let pos = position.pos;

			let entries = grid.query_rect(pos - diff, pos + diff, |other| {
				let other_id = other.inner.id;
				// Sex needed.
				if id == other_id
				{
					false
				}
				else if other.inner.solid.kind != comps::SolidKind::Demon
				{
					false
				}
				else
				{
					(other.inner.pos - position.pos).norm() < r
				}
			});

			if let Some(entry) = entries.choose(&mut rng)
			{
				if rng.gen_bool(0.5)
				{
					health.cur_health -= 1000.0;
				}
				else
				{
					let other_demon_kind =
						self.world.get::<&comps::DemonKind>(entry.inner.id).unwrap();
					let new_demon_kind = demon_kind.mate_with(*other_demon_kind);
					state.sfx.play_positional_sound(
						"data/birth.ogg",
						position.pos,
						self.camera_pos.pos,
						1.,
					)?;

					spawn_fns.push(Box::new(move |map, state| {
						spawn_demon(
							new_demon_kind,
							pos,
							Vector2::new(0., -512.),
							&mut map.world,
							state,
						)
					}));
				}
			}
		}

		// Grinder.
		for (id, (_demon_kind, position, solid)) in self
			.world
			.query::<(&comps::DemonKind, &comps::Position, &comps::Solid)>()
			.iter()
		{
			if self.tiles.get_tile_kind(
				position.pos
					+ Vector2::new(
						tiles::TILE_SIZE / 2.,
						tiles::TILE_SIZE / 2. + tiles::TILE_SIZE / 2.,
					),
			) == tiles::TileKind::Grinder
				&& solid.on_ground
			{
				state.sfx.play_positional_sound(
					"data/grind.ogg",
					position.pos,
					self.camera_pos.pos,
					1.,
				)?;

				let amount = 100;
				self.money += amount;
				self.money_change_amount = amount;
				self.money_change_time = state.hs.time();
				to_die.push(id);
			}
		}

		// Langolier.
		for (_, (position, langolier)) in self
			.world
			.query::<(&comps::Position, &mut comps::Langolier)>()
			.iter()
		{
			if let Some(player_pos) = player_pos
			{
				if (player_pos - position.pos).norm() < 16.
					&& state.hs.time() > langolier.time_to_bite
				{
					if let Ok(mut health) = self.world.get::<&mut comps::Health>(self.player)
					{
						health.cur_health -= 10.0;
						if health.cur_health > 0.
						{
							state.sfx.play_positional_sound(
								"data/pain.ogg",
								position.pos,
								self.camera_pos.pos,
								1.,
							)?;
						}
						state.sfx.play_positional_sound(
							"data/bite.ogg",
							position.pos,
							self.camera_pos.pos,
							1.,
						)?;
						langolier.time_to_bite = state.hs.time() + 0.5;
					}
				}
			}
		}

		// Health
		for (id, health) in self.world.query::<&comps::Health>().iter()
		{
			if health.cur_health < 0.
			{
				if let Some(player_pos) = player_pos
					&& id == self.player
				{
					state.sfx.play_positional_sound(
						"data/die.ogg",
						player_pos,
						self.camera_pos.pos,
						1.,
					)?;
				}
				to_die.push(id);
				if let Some((position, _)) = self
					.world
					.query_one::<(&comps::Position, &comps::Explodes)>(id)
					.unwrap()
					.get()
				{
					let pos = position.pos;
					explosions.push(pos);
					state.sfx.play_positional_sound(
						"data/explosion.ogg",
						position.pos,
						self.camera_pos.pos,
						1.,
					)?;
					state.sfx.play_positional_sound_with_dist(
						"data/explosion_far.ogg",
						position.pos,
						self.camera_pos.pos,
						10000.,
						0.75,
					)?;
					spawn_fns.push(Box::new(move |map, state| {
						spawn_explosion(pos, &mut map.world, state)
					}));
				}
			}
		}

		// Explosions.
		for pos in explosions
		{
			let damage_fn = |target_pos: Point2<f32>| {
				let f = 1. - utils::clamp((pos - target_pos).norm() / 128., 0., 1.);
				50. * f
			};
			self.tiles
				.get_tiles_in_radius(pos, 64., |tile_pos, tile_kind| match tile_kind
				{
					tiles::TileKind::Rock { health, .. } =>
					{
						*health -= damage_fn(tile_pos);
					}
					tiles::TileKind::Torch
					| tiles::TileKind::Support
					| tiles::TileKind::Jaunter => *tile_kind = tiles::TileKind::Empty,
					_ => (),
				});

			let r = 64.;
			let diff = Vector2::new(r, r);
			let entries = grid.query_rect(pos - diff, pos + diff, |other| {
				(other.inner.pos - pos).norm() < r
			});

			for entry in entries
			{
				if let Ok(health) = self
					.world
					.query_one_mut::<&mut comps::Health>(entry.inner.id)
				{
					let damage = damage_fn(entry.inner.pos);
					health.cur_health -= damage;
					if let Some(player_pos) = player_pos
						&& entry.inner.id == self.player
						&& health.cur_health > 0.
					{
						state.sfx.play_positional_sound(
							"data/pain.ogg",
							player_pos,
							self.camera_pos.pos,
							1.,
						)?;
					}
					if entry.inner.id == self.player
					{
						self.health_change_amount = -damage.ceil() as i32;
						self.health_change_time = state.hs.time();
					}
				}
			}
		}

		// Tile maintenance.
		for kill_pos in self.tiles.logic(state, self.camera_pos.pos)?
		{
			let diff = Vector2::new(tiles::TILE_SIZE, tiles::TILE_SIZE);
			let entries = grid.query_rect(kill_pos, kill_pos + diff, |other| {
				(other.inner.pos - (kill_pos + diff * 0.5)).norm() < tiles::TILE_SIZE * 0.45
			});

			for entry in entries
			{
				if let Ok(health) = self
					.world
					.query_one_mut::<&mut comps::Health>(entry.inner.id)
				{
					health.cur_health -= 1000.;
				}
			}
		}

		// DieAfterAnimationDone
		for (id, (appearance, _)) in self
			.world
			.query_mut::<(&mut comps::Appearance, &comps::DieAfterAnimationDone)>()
		{
			if appearance.animation_state.get_num_loops() > 0
			{
				to_die.push(id);
			}
		}

		// Spawn fns;
		for spawn_fn in spawn_fns
		{
			spawn_fn(self, state)?;
		}

		// Camera
		if let Ok(position) = self.world.get::<&comps::Position>(self.player)
		{
			self.camera_pos.pos += 0.25 * (position.pos - self.camera_pos.pos);
		}

		// Appearance.
		for (_, appearance) in self.world.query::<&mut comps::Appearance>().iter()
		{
			let sprite = state.get_sprite(&appearance.sprite)?;
			sprite.advance_state(
				&mut appearance.animation_state,
				(appearance.speed * DT) as f64,
			);
		}

		// Appearance animation state handling.
		for (id, (appearance, _position, acceleration, velocity)) in self
			.world
			.query::<(
				&mut comps::Appearance,
				&comps::Position,
				&comps::Acceleration,
				&comps::Velocity,
			)>()
			.iter()
		{
			if !appearance.animated
			{
				continue;
			}

			let have_item = self
				.world
				.get::<&comps::DemonHolder>(id)
				.map(|demon_holder| demon_holder.demon.is_some())
				.unwrap_or(false);
			let dir_name_str = dir_name(acceleration.last_change.x);
			let drill = self.world.get::<&comps::Drill>(id).ok();

			let mut have_drill = drill.is_some() && !have_item;
			if let Some(drill) = drill
			{
				let animation_name = if drill.want_left
				{
					"DrillLeft"
				}
				else if drill.want_right
				{
					"DrillRight"
				}
				else if drill.want_up
				{
					"DrillUp"
				}
				else if drill.want_down
				{
					"DrillDown"
				}
				else
				{
					have_drill = false;
					""
				};
				if have_drill
				{
					appearance.animation_state.set_new_animation(animation_name);
					appearance.speed = 1.;
				}
			}
			if !have_drill
			{
				if acceleration.pos.x.abs() > 1e-1
				{
					let animation_name = if have_item
					{
						format!("MoveCarry{}", dir_name_str)
					}
					else
					{
						format!("Move{}", dir_name_str)
					};
					appearance.animation_state.set_new_animation(animation_name);
					appearance.speed = velocity.pos.x.abs() / MAX_SPEED;
				}
				else
				{
					let animation_name = if have_item
					{
						format!("StandCarry{}", dir_name_str)
					}
					else
					{
						format!("Stand{}", dir_name_str)
					};
					appearance.animation_state.set_new_animation(animation_name);
					appearance.speed = 1.;
				}
			}
		}

		// Remove dead entities
		to_die.sort();
		to_die.dedup();
		for id in to_die
		{
			//println!("died {id:?}");
			self.world.despawn(id)?;
		}

		Ok(None)
	}

	fn input(
		&mut self, _event: &Event, _state: &mut game_state::GameState,
	) -> Result<Option<game_state::NextScreen>>
	{
		Ok(None)
	}

	fn draw(&mut self, state: &mut game_state::GameState) -> Result<()>
	{
		let alpha = state.hs.alpha;

		let camera_shift = self.camera_shift(alpha)
			+ Vector2::new(
				state.light_buffer.as_ref().unwrap().get_width() as f32 / 2.,
				state.light_buffer.as_ref().unwrap().get_height() as f32 / 2.,
			);

		state.hs.core.set_target_bitmap(state.light_buffer.as_ref());
		state
			.hs
			.core
			.use_shader(state.basic_shader.as_ref())
			.unwrap();
		state
			.hs
			.core
			.set_blender(BlendOperation::Add, BlendMode::One, BlendMode::Zero);
		state
			.hs
			.core
			.clear_to_color(Color::from_rgba_f(0.0, 0.0, 0.0, 0.));
		state
			.hs
			.core
			.set_blender(BlendOperation::Add, BlendMode::One, BlendMode::InverseAlpha);

		let mut batch = draw_batch::DrawBatch::new();
		self.tiles.draw(
			"data/shadow_tiles.cfg",
			Point2::origin() + camera_shift,
			&mut batch,
			state,
			false,
		)?;
		batch.draw_triangles(state);

		let mut vertices = vec![];
		let mut indices = vec![];
		for (_, (position, light)) in self.world.query_mut::<(&comps::Position, &comps::Light)>()
		{
			let draw_pos = position.draw_pos(state.hs.alpha);
			let pos = utils::round_point(
				Point2::new(draw_pos.x, draw_pos.y - light.y_offt) + camera_shift,
			);

			let rad = 16.;
			let offts = [
				Point2::new(-rad, -rad),
				Point2::new(rad, -rad),
				Point2::new(rad, rad),
				Point2::new(-rad, rad),
			];

			let idx = vertices.len() as i32;
			indices.extend([idx + 0, idx + 1, idx + 3, idx + 1, idx + 2, idx + 3]);

			for offt in offts
			{
				vertices.push(Vertex {
					x: pos.x + offt.x,
					y: pos.y + offt.y,
					z: 0.,
					u: (offt.x + rad) / (2. * rad),
					v: (offt.y + rad) / (2. * rad),
					color: light.color, //Color::from_rgb_f(r, g, b),
				});
			}
		}
		state.hs.prim.draw_indexed_prim(
			&vertices[..],
			Some(state.get_bitmap("data/circle.png").unwrap()),
			&indices[..],
			0,
			indices.len() as u32,
			PrimType::TriangleList,
		);

		let rc_buffer = game_state::light_pass(state);
		//return Ok(());

		let camera_shift = self.camera_shift(alpha)
			+ Vector2::new(state.hs.buffer_width() / 2., state.hs.buffer_height() / 2.);

		// Draw map.
		let mut batch = draw_batch::DrawBatch::new();
		self.tiles.draw(
			"data/tiles.cfg",
			Point2::origin() + camera_shift,
			&mut batch,
			state,
			true,
		)?;

		// Draw appearance.
		for (_, (appearance, position)) in self
			.world
			.query_mut::<(&comps::Appearance, &comps::Position)>()
		{
			let sprite = state.get_sprite(&appearance.sprite)?;

			let draw_pos = position.draw_pos(alpha);
			let pos = utils::round_point(draw_pos + camera_shift);

			let (atlas_bmp, offt) = sprite.get_frame_from_state(&appearance.animation_state);

			batch.add_bitmap(pos + offt, atlas_bmp, appearance.material);
		}

		state.hs.core.set_target_bitmap(state.hs.buffer1.as_ref());
		state
			.hs
			.core
			.use_shader(state.compose_shader.as_ref())
			.unwrap();
		state
			.hs
			.core
			.set_blender(BlendOperation::Add, BlendMode::One, BlendMode::InverseAlpha);
		state
			.hs
			.core
			.clear_to_color(Color::from_rgb_f(0., 0., 0.05));
		state
			.hs
			.core
			.set_shader_sampler("light", rc_buffer.unwrap(), 1)
			.unwrap();
		//.set_shader_sampler("light", state.light_buffer.as_ref().unwrap(), 1).ok();
		state
			.hs
			.core
			.set_shader_uniform(
				"light_uv_scale",
				&[[
					state.hs.buffer_width() / (state.hs.buffer_width() + game_state::RC_PAD as f32),
					state.hs.buffer_height()
						/ (state.hs.buffer_height() + game_state::RC_PAD as f32),
				]][..],
			)
			.unwrap();
		state
			.hs
			.core
			.set_shader_uniform(
				"light_uv_shift",
				&[[
					0.,
					0.,
					// game_state::RC_PAD as f32 / 2.
					// 	/ (state.hs.buffer_width() + game_state::RC_PAD as f32),
					// game_state::RC_PAD as f32 / 2.
					// 	/ (state.hs.buffer_height() + game_state::RC_PAD as f32),
				]][..],
			)
			.unwrap();
		batch.draw_triangles(state);

		state
			.hs
			.core
			.use_shader(state.basic_shader.as_ref())
			.unwrap();

		if let Ok(health) = self.world.query_one_mut::<&comps::Health>(self.player)
		{
			let pad = 8.;
			let lh = state.hs.ui_font().get_line_height() as f32 + 2.;
			let color = Color::from_rgb_f(0.8, 0.6, 0.7);

			state.hs.core.draw_text(
				state.hs.ui_font(),
				color,
				pad,
				pad,
				FontAlign::Left,
				&format!("Health: {}", health.cur_health as i32),
			);

			state.hs.core.draw_text(
				state.hs.ui_font(),
				color,
				state.hs.buffer_width() - pad,
				pad,
				FontAlign::Right,
				&format!("Money: ${}", self.money),
			);

			if self.money_change_amount != 0
			{
				let dur = 3.;
				let f = utils::clamp(
					1. - (state.hs.time() - self.money_change_time) / dur,
					0.,
					1.,
				) as f32;
				let (color, sign) = if self.money_change_amount >= 0
				{
					(Color::from_rgba_f(0.2 * f, 0.6 * f, 0.1 * f, f), "+")
				}
				else
				{
					(Color::from_rgba_f(0.6 * f, 0.2 * f, 0.1 * f, f), "-")
				};

				state.hs.core.draw_text(
					state.hs.ui_font(),
					color,
					state.hs.buffer_width() - pad,
					pad + lh,
					FontAlign::Right,
					&format!("{}${}", sign, self.money_change_amount.abs()),
				);
			}

			if self.health_change_time > 0.
			{
				let dur = 3.;
				let f = utils::clamp(
					1. - (state.hs.time() - self.health_change_time) / dur,
					0.,
					1.,
				) as f32;
				let (color, sign) = if self.health_change_amount >= 0
				{
					(Color::from_rgba_f(0.2 * f, 0.6 * f, 0.1 * f, f), "+")
				}
				else
				{
					(Color::from_rgba_f(0.6 * f, 0.2 * f, 0.1 * f, f), "-")
				};

				state.hs.core.draw_text(
					state.hs.ui_font(),
					color,
					pad,
					pad + lh,
					FontAlign::Left,
					&format!("{}{}", sign, self.health_change_amount.abs()),
				);
			}
		}

		//self.tiles
		//	.draw_support(Point2::origin() + camera_shift, state)?;

		Ok(())
	}

	fn camera_to_world(&self, pos: Point2<f32>, state: &game_state::GameState) -> Point2<f32>
	{
		self.camera_pos.pos + pos.coords
			- Vector2::new(state.hs.buffer_width() / 2., state.hs.buffer_height() / 2.)
	}

	fn camera_shift(&self, alpha: f32) -> Vector2<f32>
	{
		-self.camera_pos.draw_pos(alpha).coords
	}
}
