// D3D 各 pipeline 共享的圆角 SDF 数学——由 RECT_HLSL 与 SHADOW_HLSL 经
// concat!(include_str!()) 各引入一次编译，禁止在 pipeline 源内再各持一份。
// Port of CPU `rounded_rect_sdf` (center-relative, per-corner radius).
float rounded_rect_sdf(float2 local, float2 size, float4 radius)
{
    float2 half_size = size * 0.5;
    float2 q = local - half_size;
    float cr;
    if (q.x < 0.0)
        cr = (q.y < 0.0) ? radius.x : radius.w;
    else
        cr = (q.y < 0.0) ? radius.y : radius.z;
    float2 d = abs(q) - half_size + cr;
    float outside = length(max(d, 0.0));
    float inside = min(max(d.x, d.y), 0.0);
    return outside + inside - cr;
}
