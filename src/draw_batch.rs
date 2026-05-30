use crate::game_state;

use allegro::*;
use allegro_primitives::*;
use nalgebra::Point2;

use slhack::atlas;

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct BatchVertex
{
	x: f32,
	y: f32,
	z: f32,
	u: f32,
	v: f32,
	material: f32,
	color: Color,
}

unsafe impl VertexType for BatchVertex
{
	fn get_decl(prim: &PrimitivesAddon) -> VertexDecl
	{
		fn make_builder() -> std::result::Result<VertexDeclBuilder, ()>
		{
			VertexDeclBuilder::new(std::mem::size_of::<BatchVertex>())
				.pos(
					VertexAttrStorage::F32_3,
					memoffset::offset_of!(BatchVertex, x),
				)?
				.uv(
					VertexAttrStorage::F32_2,
					memoffset::offset_of!(BatchVertex, u),
				)?
				.color(memoffset::offset_of!(BatchVertex, color))?
				.user_attr(
					VertexAttrStorage::F32_1,
					memoffset::offset_of!(BatchVertex, material),
				)
		}

		VertexDecl::from_builder(prim, &make_builder().unwrap())
	}
}

struct Batch
{
	vertices: Vec<BatchVertex>,
	indices: Vec<i32>,
}

pub struct DrawBatch
{
	batches: Vec<Batch>,
}

impl DrawBatch
{
	pub fn new() -> Self
	{
		DrawBatch { batches: vec![] }
	}

	fn ensure_batch(&mut self, page: usize)
	{
		while page >= self.batches.len()
		{
			self.batches.push(Batch {
				vertices: vec![],
				indices: vec![],
			});
		}
	}

	fn add_vertices(&mut self, vertices: &[BatchVertex], page: usize)
	{
		self.ensure_batch(page);
		self.batches[page].vertices.extend(vertices);
	}

	fn add_indices(&mut self, indices: &[i32], page: usize)
	{
		self.ensure_batch(page);
		self.batches[page].indices.extend(indices);
	}

	fn add_vertex(&mut self, vertex: BatchVertex, page: usize)
	{
		self.ensure_batch(page);
		self.batches[page].vertices.push(vertex);
	}

	fn add_index(&mut self, index: i32, page: usize)
	{
		self.ensure_batch(page);
		self.batches[page].indices.push(index);
	}

	fn num_vertices(&mut self, page: usize) -> i32
	{
		self.ensure_batch(page);
		self.batches[page].vertices.len() as i32
	}

	pub fn add_bitmap(
		&mut self, pos: Point2<f32>, bmp: atlas::AtlasBitmap, material: game_state::MaterialKind,
	)
	{
		let color = Color::from_rgb_f(1., 1., 1.);
		let page_size = 1024.;
		let vertices = [
			BatchVertex {
				x: pos.x,
				y: pos.y,
				z: 0.,
				u: bmp.start.x / page_size,
				v: 1. - bmp.start.y / page_size,
				color: color,
				material: material as i32 as f32,
			},
			BatchVertex {
				x: pos.x + bmp.width(),
				y: pos.y,
				z: 0.,
				u: bmp.end.x / page_size,
				v: 1. - bmp.start.y / page_size,
				color: color,
				material: material as i32 as f32,
			},
			BatchVertex {
				x: pos.x + bmp.width(),
				y: pos.y + bmp.height(),
				z: 0.,
				u: bmp.end.x / page_size,
				v: 1. - bmp.end.y / page_size,
				color: color,
				material: material as i32 as f32,
			},
			BatchVertex {
				x: pos.x,
				y: pos.y + bmp.height(),
				z: 0.,
				u: bmp.start.x / page_size,
				v: 1. - bmp.end.y / page_size,
				color: color,
				material: material as i32 as f32,
			},
		];
		let idx = self.num_vertices(bmp.page);
		let indices = [idx + 0, idx + 1, idx + 2, idx + 0, idx + 2, idx + 3];
		self.add_indices(&indices[..], bmp.page);
		self.add_vertices(&vertices[..], bmp.page);
	}

	pub fn draw_triangles(&self, state: &game_state::GameState)
	{
		for (i, page) in state.atlas.pages.iter().enumerate()
		{
			if i >= self.batches.len()
			{
				break;
			}
			state.hs.prim.draw_indexed_prim(
				&self.batches[i].vertices[..],
				Some(&page.bitmap),
				&self.batches[i].indices[..],
				0,
				self.batches[i].indices.len() as u32,
				PrimType::TriangleList,
			);
		}
	}
}
