use crate::error::{Result, ResultHelper};
use crate::game_state::DT;
use crate::{components as comps, draw_batch, game_state, tiles, ui, utils};
use allegro::*;
use allegro_font::*;
use nalgebra::{Point2, Vector2};
use rand::prelude::*;
use slhack::{controls, scene, spatial_grid, sprite, ui as slhack_ui};

use std::collections::HashMap;
use std::f32::consts::PI;

const MAX_SPEED: f32 = 150.0;
const PICKUP_RADIUS: f32 = 16.0;
const DRILL_RADIUS: f32 = 24.0;

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
		comps::Solid::new(24., comps::SolidKind::Player),
		comps::Gravity,
		comps::DemonHolder::new(),
		comps::Drill::new(),
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
		comps::Gravity,
		kind,
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

		let pos = Point2::new(tiles::TILE_SIZE * 12., tiles::TILE_SIZE * 10.);
		spawn_demon(
			comps::DemonKind::Demon1,
			pos,
			Vector2::zeros(),
			&mut world,
			state,
		)?;
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

		// XXX: Why?
		for (_, acceleration) in self.world.query_mut::<&mut comps::Acceleration>()
		{
			acceleration.pos = Vector2::zeros();
		}

		let want_move_left = state
			.controls
			.get_action_state(game_state::Action::MoveLeft);
		let want_move_right = state
			.controls
			.get_action_state(game_state::Action::MoveRight);
		let want_jump = state.controls.get_action_state(game_state::Action::Jump) > 0.5;
		let want_pickup = state.controls.get_action_state(game_state::Action::Pickup) > 0.5;
		state
			.controls
			.clear_action_state(game_state::Action::Pickup);
		let want_drill_left = state
			.controls
			.get_action_state(game_state::Action::DrillLeft)
			> 0.5;
		let want_drill_right = state
			.controls
			.get_action_state(game_state::Action::DrillRight)
			> 0.5;
		let want_drill_up = state.controls.get_action_state(game_state::Action::DrillUp) > 0.5;
		let want_drill_down = state
			.controls
			.get_action_state(game_state::Action::DrillDown)
			> 0.5;
		let want_drill = want_drill_left || want_drill_right || want_drill_down || want_drill_up;

		if self.world.contains(self.player)
		{
			let right_left = want_move_right - want_move_left;
			if let Ok((solid, acceleration, velocity, drill)) = self.world.query_one_mut::<(
				&comps::Solid,
				&mut comps::Acceleration,
				&mut comps::Velocity,
				&mut comps::Drill,
			)>(self.player)
			{
				let control = if solid.on_ground { 1. } else { 0.5 };
				let can_move = !want_drill;

				if can_move
				{
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

				drill.want_left = want_drill_left;
				drill.want_right = want_drill_right;
				drill.want_up = want_drill_up;
				drill.want_down = want_drill_down;
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
		for (_id, (position, velocity, solid)) in self
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
				solid.on_ground = escape_dir.y < -1e-3;
				if solid.on_ground
				{
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

		// Pickup.
		// XXX: Weird how this happens after all the other stuff...
		if self.world.contains(self.player)
		{
			if want_pickup
			{
				let r = PICKUP_RADIUS;
				let mut do_spawn_demon = None;
				let mut pickup_demon = None;
				if let Ok((position, velocity, acceleration, demon_holder)) =
					self.world.query_one_mut::<(
						&mut comps::Position,
						&comps::Velocity,
						&comps::Acceleration,
						&mut comps::DemonHolder,
					)>(self.player)
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

					if let Some(demon_entry) = entries.iter().copied().next()
					{
						let item_pos = position.pos
							+ Vector2::new(
								PICKUP_RADIUS * acceleration.last_change.x.signum(),
								-8.,
							);
						pickup_demon = Some((demon_entry.inner.id, item_pos));
					}
					if let Some(demon_item_id) = demon_holder.demon.take()
					{
						do_spawn_demon = Some((
							demon_item_id,
							position.pos,
							velocity.pos,
							acceleration.last_change.x.signum(),
						));
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
		for (_, (position, drill)) in self
			.world
			.query::<(&comps::Position, &comps::Drill)>()
			.iter()
		{
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
					let new_tile = match tile
					{
						tiles::TileKind::Rock { health } =>
						{
							let new_health = *health - 75. * DT;
							if new_health <= 0.
							{
								tiles::TileKind::Empty
							}
							else
							{
								tiles::TileKind::Rock { health: new_health }
							}
						}
						tiles::TileKind::Empty => tiles::TileKind::Empty,
					};
					*tile = new_tile;
				}
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
		for (id, (appearance, position, acceleration, velocity)) in self
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

			let mut have_drill = drill.is_some();
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

			let draw_pos = position.draw_pos(alpha);
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
