uniform sampler2D al_tex;
varying vec4 varying_color;
varying vec2 varying_texcoord;
varying float varying_material;
varying vec4 varying_pos;

uniform vec2 light_uv_shift;
uniform vec2 light_uv_scale;
uniform sampler2D light;

void main()
{
	vec4 color = texture2D(al_tex, varying_texcoord);
	int material = int(varying_material);

	vec4 light_color = vec4(1.);
	//if (material == LIT_MATERIAL)
	{
		light_color = 4. * texture2D(light, 0.5 * (light_uv_scale * varying_pos.xy + light_uv_shift) + vec2(0.5, 0.5));
		light_color = 2. * light_color;
		light_color = vec4(light_color.rgb, 1.);
	}

	gl_FragColor = light_color * varying_color * color;
}

