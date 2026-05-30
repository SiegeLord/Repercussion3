use crate::error::Result;
use crate::{ui, utils};
use allegro::*;
use allegro_font::*;
use allegro_image::*;
use allegro_primitives::*;
use allegro_ttf::*;
use nalgebra::{Point2, Vector2};
use serde_derive::{Deserialize, Serialize};
use slhack::hack_state::HackState;
use slhack::{atlas, controls, deferred, hack_state, scene, sfx, sprite};
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap};
use std::{fmt, path, sync};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DT: f32 = 1. / 60.;

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
#[repr(i32)]
pub enum MaterialKind
{
	Default = 0,
	Lit = 1,
	NumMaterials = 2,
}

pub fn shader_replacements() -> Vec<(&'static str, &'static str)>
{
	let mut ret = vec![];
	for i in 0..MaterialKind::NumMaterials as i32
	{
		let variant = unsafe { std::mem::transmute(i) };
		ret.push(match variant
		{
			MaterialKind::Default => ("DEFAULT_MATERIAL", "0"),
			MaterialKind::Lit => ("LIT_MATERIAL", "1"),
			MaterialKind::NumMaterials => unreachable!(),
		});
	}
	ret
}

impl Into<i32> for MaterialKind
{
	fn into(self) -> i32
	{
		self as i32
	}
}

slhack::actions! {
	Action
	{
		MoveLeft = [Some(controls::Input::Keyboard(KeyCode::Left)), Some(controls::Input::JoystickNegAxis(JoystickStick::LeftThumb, 0))],
		MoveRight = [Some(controls::Input::Keyboard(KeyCode::Right)), Some(controls::Input::JoystickNegAxis(JoystickStick::LeftThumb, 1))],
		Jump = [Some(controls::Input::Keyboard(KeyCode::Space)), Some(controls::Input::JoystickButton(JoystickButton::A))],
		Pickup = [Some(controls::Input::Keyboard(KeyCode::LShift)), Some(controls::Input::JoystickButton(JoystickButton::B))],
		DrillLeft = [Some(controls::Input::Keyboard(KeyCode::A)), Some(controls::Input::JoystickNegAxis(JoystickStick::DPad, 0))],
		DrillRight = [Some(controls::Input::Keyboard(KeyCode::D)), Some(controls::Input::JoystickPosAxis(JoystickStick::DPad, 0))],
		DrillUp = [Some(controls::Input::Keyboard(KeyCode::W)), Some(controls::Input::JoystickNegAxis(JoystickStick::DPad, 1))],
		DrillDown = [Some(controls::Input::Keyboard(KeyCode::S)), Some(controls::Input::JoystickPosAxis(JoystickStick::DPad, 1))],
	}
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Options
{
	pub version: String,
	pub gfx: hack_state::GfxOptions,

	pub play_music: bool,
	pub sfx_volume: f32,
	pub music_volume: f32,
	pub camera_speed: f32,
	pub controls: controls::Controls<Action>,
}

impl Default for Options
{
	fn default() -> Self
	{
		Self {
			version: VERSION.to_string(),
			gfx: hack_state::GfxOptions {
				fullscreen: false,
				width: 640 * 2,
				height: 360 * 2,
				vsync_method: if cfg!(target_os = "windows") { 1 } else { 2 },
				grab_mouse: false,
				ui_scale: 1.,
				frac_scale: true,
			},
			play_music: false,
			sfx_volume: 1.,
			music_volume: 1.,
			camera_speed: 2.,
			controls: Action::new_controls(),
		}
	}
}

type Scene = scene::Scene<MaterialKind>;

#[derive(Debug)]
pub enum NextScreen
{
	Game,
	Menu,
	InGameMenu,
	Quit,
}

pub struct GameState
{
	pub sfx: sfx::Sfx,
	pub atlas: atlas::Atlas,
	pub options: Options,
	pub controls: controls::ControlsHandler<Action>,

	pub light_buffer: Option<Bitmap>,
	pub ray_casting_buffer_1: Option<Bitmap>,
	pub ray_casting_buffer_2: Option<Bitmap>,
	pub distance_buffer_1: Option<Bitmap>,
	pub distance_buffer_2: Option<Bitmap>,
	pub distance_buffer_fin: Option<Bitmap>,

	pub basic_shader: Option<Shader>,
	pub compose_shader: Option<Shader>,
	pub jfa_seed_shader: Option<Shader>,
	pub jfa_jump_shader: Option<Shader>,
	pub jfa_dist_shader: Option<Shader>,
	pub ray_casting_shader: Option<Shader>,

	bitmaps: HashMap<String, Bitmap>,
	sprites: HashMap<String, sprite::Sprite>,

	// Has to be last!
	pub hs: hack_state::HackState,
}

pub fn load_options(core: &Core) -> Result<Options>
{
	Ok(
		utils::load_user_data_deferred(core, "options.cfg", |user_data| {
			let version = semver::Version::parse(VERSION).unwrap();
			if let Some(config_version) = user_data.version.as_ref()
			{
				if version.major == config_version.major && version.minor == config_version.minor
				{
					user_data.parse()
				}
				else
				{
					// TODO: Migrate previous versions.
					Ok(None)
				}
			}
			else
			{
				Ok(None)
			}
		})?
		.unwrap_or_default(),
	)
}

pub fn save_options(core: &Core, options: &Options) -> Result<()>
{
	Ok(utils::save_user_data(core, "options.cfg", options)?)
}

impl GameState
{
	pub fn new() -> Result<Self>
	{
		let mut options = Options::default();
		let hack_load_options = |core: &Core| -> slhack::error::Result<hack_state::GfxOptions> {
			options = load_options(core).map_err(Into::<slhack::error::Error>::into)?;
			Ok(options.gfx.clone())
		};
		let hack_state =
			hack_state::HackState::new("Repercussion 3", hack_load_options, Some((640, 360)))?;

		let sfx = sfx::Sfx::new(options.sfx_volume, options.music_volume, &hack_state.core)?;
		//sfx.set_music_file("data/lemonade-sinus.xm");
		//sfx.play_music()?;

		let controls = controls::ControlsHandler::new(options.controls.clone(), 1.);
		Ok(Self {
			options: options,
			bitmaps: HashMap::new(),
			sprites: HashMap::new(),
			sfx: sfx,
			atlas: atlas::Atlas::new(1024),
			controls: controls,
			basic_shader: None,
			compose_shader: None,
			jfa_seed_shader: None,
			jfa_jump_shader: None,
			jfa_dist_shader: None,
			ray_casting_shader: None,
			light_buffer: None,
			ray_casting_buffer_1: None,
			ray_casting_buffer_2: None,
			distance_buffer_1: None,
			distance_buffer_2: None,
			distance_buffer_fin: None,
			hs: hack_state,
		})
	}

	pub fn resize_display(&mut self) -> Result<()>
	{
		self.hs
			.resize_display("data/Energon.ttf", -16.0, &self.options.gfx)?;

		let buffer_width = self.hs.buffer_width() as i32;
		let buffer_height = self.hs.buffer_height() as i32;

		let old_flags = self.hs.core.get_new_bitmap_flags();
		self.hs.core.set_new_bitmap_flags(MAG_LINEAR | MIN_LINEAR);
		self.light_buffer = Some(Bitmap::new(&self.hs.core, buffer_width, buffer_height).unwrap());
		self.ray_casting_buffer_1 =
			Some(Bitmap::new(&self.hs.core, buffer_width, buffer_height).unwrap());
		self.ray_casting_buffer_2 =
			Some(Bitmap::new(&self.hs.core, buffer_width, buffer_height).unwrap());
		self.hs.core.set_new_bitmap_flags(old_flags);

		let old_format = self.hs.core.get_new_bitmap_format();
		self.hs.core.set_new_bitmap_format(PixelFormat::AbgrF32);
		self.distance_buffer_1 =
			Some(Bitmap::new(&self.hs.core, buffer_width, buffer_height).unwrap());
		self.distance_buffer_2 =
			Some(Bitmap::new(&self.hs.core, buffer_width, buffer_height).unwrap());
		self.hs.core.set_new_bitmap_format(old_format);

		self.distance_buffer_fin =
			Some(Bitmap::new(&self.hs.core, buffer_width, buffer_height).unwrap());

		Ok(())
	}

	pub fn cache_bitmap<'l>(&'l mut self, name: &str) -> Result<&'l Bitmap>
	{
		Ok(match self.bitmaps.entry(name.to_string())
		{
			Entry::Occupied(o) => o.into_mut(),
			Entry::Vacant(v) => v.insert(utils::load_bitmap(&self.hs.core, name)?),
		})
	}

	pub fn cache_sprite<'l>(&'l mut self, name: &str) -> Result<&'l sprite::Sprite>
	{
		Ok(match self.sprites.entry(name.to_string())
		{
			Entry::Occupied(o) => o.into_mut(),
			Entry::Vacant(v) =>
			{
				v.insert(sprite::Sprite::load(name, &self.hs.core, &mut self.atlas)?)
			}
		})
	}

	pub fn get_bitmap<'l>(&'l self, name: &str) -> Result<&'l Bitmap>
	{
		Ok(self
			.bitmaps
			.get(name)
			.ok_or_else(|| format!("{name} is not cached!"))?)
	}

	pub fn get_sprite<'l>(&'l self, name: &str) -> Result<&'l sprite::Sprite>
	{
		Ok(self
			.sprites
			.get(name)
			.ok_or_else(|| format!("{name} is not cached!"))?)
	}
}

pub fn light_pass(state: &GameState) -> Option<&Bitmap>
{
	let core = &state.hs.core;

	core.set_blender(BlendOperation::Add, BlendMode::One, BlendMode::Zero);
	// Seed distance buffer
	core.set_target_bitmap(state.distance_buffer_1.as_ref());
	core.use_shader(state.basic_shader.as_ref()).unwrap();
	state
		.hs
		.core
		.clear_to_color(Color::from_rgb_f(0.0, 0.0, 0.0));

	let buffer_size = Vector2::new(state.hs.buffer_width(), state.hs.buffer_height());
	core.use_shader(state.jfa_seed_shader.as_ref()).unwrap();
	core.set_shader_uniform("bitmap_size", &[[buffer_size.x, buffer_size.y]][..])
		.ok();
	core.draw_bitmap(state.light_buffer.as_ref().unwrap(), 0., 0., Flag::zero());

	// JFA
	let num_passes = utils::max(buffer_size.x, buffer_size.y).log2().ceil() as i32;
	let buffers = [
		state.distance_buffer_1.as_ref(),
		state.distance_buffer_2.as_ref(),
	];
	for i in 0..num_passes
	{
		let src_buffer = buffers[(i % 2) as usize];
		let dst_buffer = buffers[(1 - i % 2) as usize];
		core.set_target_bitmap(dst_buffer);
		core.use_shader(state.jfa_jump_shader.as_ref()).unwrap();
		core.set_shader_uniform("bitmap_size", &[[buffer_size.x, buffer_size.y]][..])
			.ok();
		core.set_shader_uniform(
			"uv_offset",
			&[2.0_f32.powf((num_passes - i - 1) as f32)][..],
		)
		.ok();
		core.draw_bitmap(src_buffer.unwrap(), 0., 0., Flag::zero());
	}
	let src_buffer = buffers[(num_passes % 2) as usize];
	core.set_target_bitmap(state.distance_buffer_fin.as_ref());
	core.use_shader(state.jfa_dist_shader.as_ref()).unwrap();
	core.draw_bitmap(src_buffer.unwrap(), 0., 0., Flag::zero());

	// Ray casting.
	let rc_buffer;
	if false
	{
		core.set_target_bitmap(state.ray_casting_buffer_1.as_ref());
		core.use_shader(state.ray_casting_shader.as_ref()).unwrap();
		core.set_shader_uniform("num_rays", &[128][..]).unwrap();
		core.set_shader_uniform("num_steps", &[32][..]).unwrap();
		core.set_shader_sampler(
			"distance_map",
			state.distance_buffer_fin.as_ref().unwrap(),
			2,
		)
		.ok();
		core.draw_bitmap(state.light_buffer.as_ref().unwrap(), 0., 0., Flag::zero());
		rc_buffer = state.ray_casting_buffer_1.as_ref();
	}
	else
	{
		let buffers = [
			state.ray_casting_buffer_1.as_ref(),
			state.ray_casting_buffer_2.as_ref(),
		];
		let diag = buffer_size.norm();
		let base = 4.0_f32;
		let num_cascades = ((diag.ln() / base.ln()).ceil() + 1.) as i32;

		//let last_idx = num_cascades - 1;
		let last_idx = 0;
		for i in (last_idx..=num_cascades - 1).rev()
		{
			let src_buffer = buffers[(i % 2) as usize];
			let dst_buffer = buffers[(1 - i % 2) as usize];
			core.set_target_bitmap(dst_buffer);
			core.use_shader(state.ray_casting_shader.as_ref()).unwrap();
			core.set_shader_sampler(
				"distance_map",
				state.distance_buffer_fin.as_ref().unwrap(),
				2,
			)
			.ok();
			core.set_shader_sampler("prev_cascade", src_buffer.unwrap(), 3)
				.ok();
			core.set_shader_uniform("base", &[base][..]).ok();
			core.set_shader_uniform("bitmap_size", &[[buffer_size.x, buffer_size.y]][..])
				.ok();
			core.set_shader_uniform("cascade_index", &[i as f32][..])
				.ok();
			core.set_shader_uniform("num_cascades", &[num_cascades as f32][..])
				.ok();
			core.set_shader_uniform("last_index", &[(i == last_idx) as i32][..])
				.ok();
			core.set_shader_uniform("num_steps", &[32_i32][..]).ok();
			core.draw_bitmap(state.light_buffer.as_ref().unwrap(), 0., 0., Flag::zero());
		}
		rc_buffer = buffers[1 - last_idx as usize % 2];
	}

	// Debug
	if false
	{
		state.hs.core.set_target_bitmap(state.hs.buffer1.as_ref());
		state
			.hs
			.core
			.use_shader(Some(&*state.basic_shader.as_ref().unwrap()))
			.unwrap();
		state
			.hs
			.core
			.clear_to_color(Color::from_rgb_f(1.0, 1.0, 1.0));
		state
			.hs
			.core
			.set_blender(BlendOperation::Add, BlendMode::One, BlendMode::InverseAlpha);
		state.hs.core.draw_bitmap(
			//buffers[num_passes as usize % 2].unwrap(),
			//state.light_buffer.as_ref().unwrap(),
			//state.distance_buffer_fin.as_ref().unwrap(),
			rc_buffer.unwrap(),
			0.,
			0.,
			Flag::zero(),
		);
	}
	rc_buffer
}
