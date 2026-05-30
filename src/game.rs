use crate::error::{Result, ResultHelper};
use crate::game_state::DT;
use crate::{components as comps, draw_batch, game_state, tiles, ui, utils};
use allegro::*;
use allegro_font::*;
use nalgebra::{Point2, Vector2};
use rand::prelude::*;
use slhack::{controls, scene, sprite, ui as slhack_ui};

use std::collections::HashMap;
use std::f32::consts::PI;

const MAX_SPEED: f32 = 150.0;

pub struct Game
{
	map: Map,
	subscreens: ui::SubScreens,
}

impl Game
{
	pub fn new(state: &mut game_state::GameState) -> Result<Self>
	{
		Ok(Self {
			map: Map::new(state)?,
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
		comps::Solid::new(24.),
		comps::Gravity,
	));

	Ok(entity)
}

fn dir_name(vx: f32) -> &'static str
{
	if vx > 0. { "Right" } else { "Left" }
}

struct Map
{
	world: hecs::World,
	tiles: tiles::Tiles,
	camera_pos: comps::Position,
	player: hecs::Entity,
}

impl Map
{
	fn new(state: &mut game_state::GameState) -> Result<Self>
	{
		let mut world = hecs::World::new();
		state.cache_sprite("data/tiles.cfg")?;

		let pos = Point2::new(tiles::TILE_SIZE * 10., tiles::TILE_SIZE * 10.);
		let player = spawn_player(pos, &mut world, state)?;

		Ok(Self {
			world: world,
			tiles: tiles::Tiles::new(16, 16)?,
			camera_pos: comps::Position::new(Point2::origin()),
			player: player,
		})
	}

	fn logic(&mut self, state: &mut game_state::GameState)
	-> Result<Option<game_state::NextScreen>>
	{
		let mut to_die = vec![];

		// Position snapshotting.
		for (_, position) in self.world.query::<&mut comps::Position>().iter()
		{
			position.snapshot();
		}
		self.camera_pos.snapshot();

		let want_move_left = state
			.controls
			.get_action_state(game_state::Action::MoveLeft);
		let want_move_right = state
			.controls
			.get_action_state(game_state::Action::MoveRight);
		let want_jump = state.controls.get_action_state(game_state::Action::Jump) > 0.5;

		if self.world.contains(self.player)
		{
			let right_left = want_move_right - want_move_left;
			if let Ok((solid, acceleration, velocity)) = self.world.query_one_mut::<(
				&comps::Solid,
				&mut comps::Acceleration,
				&mut comps::Velocity,
			)>(self.player)
			{
				let control = if solid.on_ground { 1. } else { 0.5 };
				acceleration.pos.x = 256. * right_left * control;
				if right_left.abs() > 1e-1
				{
					acceleration.last_change = acceleration.pos;
				}
				if want_jump && (state.hs.time() - solid.last_on_ground) < 0.2
				{
					velocity.pos.y -= 64.;
					//println!("Jump: {}", velocity.pos.y);
				}
			}
		}

		// Input.
		//if state.controls.get_action_state(game_state::Action::Move) > 0.5
		//{
		//	for (_, position) in self.world.query::<&mut comps::Position>().iter()
		//	{
		//		position.pos.y += 100. * DT;
		//	}
		//}

		// Friction.
		for (_, (velocity, acceleration, solid)) in self
			.world
			.query::<(
				&mut comps::Velocity,
				&mut comps::Acceleration,
				&comps::Solid,
			)>()
			.iter()
		{
			if solid.on_ground && acceleration.pos.x.abs() == 0.
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
		for (_, acceleration) in self.world.query::<&mut comps::Acceleration>().iter()
		{
			acceleration.pos.y = 512.;
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
		for (_, (position, velocity, solid)) in self
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
				tiles::TileKind::Rock,
			)
			{
				position.pos += escape_dir;
				solid.on_ground = escape_dir.y < -1e-3;
				if solid.on_ground
				{
					solid.last_on_ground = state.hs.time();
				}
				let norm_escape_dir = escape_dir.normalize();
				if velocity.pos.norm() > 0.
				{
					let proj_velocity = norm_escape_dir * velocity.pos.dot(&norm_escape_dir);
					//println!("ed: {:?} v: {:?}, pv: {:?}", escape_dir, velocity, proj_velocity);
					velocity.pos -= proj_velocity;
				}
			}
			else
			{
				solid.on_ground = false;
			}
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
		for (_, (appearance, position, acceleration, velocity)) in self
			.world
			.query::<(
				&mut comps::Appearance,
				&comps::Position,
				&comps::Acceleration,
				&comps::Velocity,
			)>()
			.iter()
		{
			if acceleration.pos.x.abs() > 1e-1
			{
				appearance
					.animation_state
					.set_new_animation(format!("Move{}", dir_name(acceleration.last_change.x)));
				appearance.speed = velocity.pos.x.abs() / MAX_SPEED;
			}
			else
			{
				appearance
					.animation_state
					.set_new_animation(format!("Stand{}", dir_name(acceleration.last_change.x)));
				appearance.speed = 1.;
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
		state
			.hs
			.core
			.clear_to_color(Color::from_rgb_f(0., 0., 0.05));

		let mut batch = draw_batch::DrawBatch::new();

		let camera_shift = self.camera_shift(alpha, state);

		// Draw map.
		self.tiles
			.draw(Point2::origin() + camera_shift, &mut batch, state, false)?;

		// Draw appearance.
		for (_, (appearance, position)) in self
			.world
			.query_mut::<(&comps::Appearance, &comps::Position)>()
		{
			let sprite = state.get_sprite(&appearance.sprite)?;

			let draw_pos = position.draw_pos(state.hs.alpha);
			let pos = utils::round_point(draw_pos + camera_shift);

			let (atlas_bmp, offt) = sprite.get_frame_from_state(&appearance.animation_state);

			batch.add_bitmap(pos + offt, atlas_bmp, appearance.material);
		}

		state
			.hs
			.core
			.use_shader(state.compose_shader.as_ref())
			.unwrap();
		batch.draw_triangles(state);

		state
			.hs
			.core
			.use_shader(state.basic_shader.as_ref())
			.unwrap();
		Ok(())
	}

	fn camera_to_world(&self, pos: Point2<f32>, state: &game_state::GameState) -> Point2<f32>
	{
		self.camera_pos.pos.xy() + pos.coords
			- Vector2::new(state.hs.buffer_width() / 2., state.hs.buffer_height() / 2.)
	}

	fn camera_shift(&self, alpha: f32, state: &game_state::GameState) -> Vector2<f32>
	{
		-self.camera_pos.draw_pos(alpha).xy().coords
			+ Vector2::new(state.hs.buffer_width() / 2., state.hs.buffer_height() / 2.)
	}
}
