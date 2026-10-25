#version 100
precision mediump float;

varying vec2 uv;

uniform sampler2D _ScreenTexture;
uniform float Time;

void main() {
    vec2 p = uv * 2.0 - 1.0;
    vec2 curved = p * (1.0 + 0.018 * dot(p, p));
    vec2 screen_uv = (curved + 1.0) * 0.5;
    screen_uv.y = 1.0 - screen_uv.y;

    if (screen_uv.x < 0.0 || screen_uv.x > 1.0 || screen_uv.y < 0.0 || screen_uv.y > 1.0) {
        gl_FragColor = vec4(0.004, 0.016, 0.035, 1.0);
        return;
    }

    vec2 offset = vec2(0.00055, 0.0);
    float red = texture2D(_ScreenTexture, screen_uv + offset).r;
    float green = texture2D(_ScreenTexture, screen_uv).g;
    float blue = texture2D(_ScreenTexture, screen_uv - offset).b;
    vec3 color = vec3(red, green, blue);

    float scan = 0.05 * (0.5 + 0.5 * sin(screen_uv.y * 1200.0));
    color *= 1.0 - scan;

    float vignette = smoothstep(1.45, 0.15, length(p));
    color *= 0.82 + 0.18 * vignette;

    gl_FragColor = vec4(color, 1.0);
}
