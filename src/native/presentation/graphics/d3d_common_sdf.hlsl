// D3D 各 pipeline 共享的圆角 SDF 数学——由 RECT_HLSL 与 SHADOW_HLSL 经
// concat!(include_str!()) 各引入一次编译，禁止在 pipeline 源内再各持一份。
// Port of CPU `rounded_rect_sdf`（直角矩形与各角弧约束的交集）：
// 直边距离为基线，落入切线方形的每个角弧距离（圆外为正）取最大。
// 归一化只保证相邻切线方形不相交，对角方形可重叠（如 100x100 的
// tl=br=80，重叠区须同时满足两角约束）。local 为相对矩形左上角坐标，
// radius 分量顺序 tl/tr/br/bl。
float rounded_rect_sdf(float2 local, float2 size, float4 radius)
{
    float2 q = abs(local - size * 0.5) - size * 0.5;
    float combined = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0);
    if (radius.x > 0.0 && local.x < radius.x && local.y < radius.x)
        combined = max(combined, distance(local, float2(radius.x, radius.x)) - radius.x);
    if (radius.y > 0.0 && local.x > size.x - radius.y && local.y < radius.y)
        combined = max(combined, distance(local, float2(size.x - radius.y, radius.y)) - radius.y);
    if (radius.z > 0.0 && local.x > size.x - radius.z && local.y > size.y - radius.z)
        combined = max(combined, distance(local, size - radius.z) - radius.z);
    if (radius.w > 0.0 && local.x < radius.w && local.y > size.y - radius.w)
        combined = max(combined, distance(local, float2(radius.w, size.y - radius.w)) - radius.w);
    return combined;
}
