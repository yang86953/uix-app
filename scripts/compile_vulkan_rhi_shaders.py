"""从唯一 GLSL 源生成并校验 Vulkan 薄 RHI 的固定 SPIR-V 变体。"""

from pathlib import Path
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
SHADERS = ROOT / "src/native/presentation/graphics/vulkan/adapter/shaders"
OUTPUT = SHADERS / "spv"
# GLSL 圆角/阴影数学的唯一共享定义，注入每个变体的 #version 行之后。
COMMON_SDF = (
    ROOT / "src/native/presentation/graphics/glsl_common_sdf.frag"
).read_text()

VERTEX_VARIANTS = {
    "mesh.vert.spv": "UIX_MESH",
    "sampled.vert.spv": "UIX_SAMPLED",
    "gradient.vert.spv": "UIX_GRADIENT",
    "shape.vert.spv": "UIX_SHAPE",
    "shadow.vert.spv": "UIX_SHADOW",
    "blur.vert.spv": "UIX_BLUR",
    "msdf.vert.spv": "UIX_MSDF",
    "sector.vert.spv": "UIX_SECTOR",
    "line.vert.spv": "UIX_LINE",
}

FRAGMENT_VARIANTS = {
    "mesh.frag.spv": "UIX_MESH",
    "textured.frag.spv": "UIX_TEXTURED",
    "coverage.frag.spv": "UIX_COVERAGE",
    "gradient.frag.spv": "UIX_GRADIENT",
    "shape.frag.spv": "UIX_SHAPE",
    "shadow.frag.spv": "UIX_SHADOW",
    "blur.frag.spv": "UIX_BLUR",
    "msdf.frag.spv": "UIX_MSDF",
    "sector.frag.spv": "UIX_SECTOR",
    "line.frag.spv": "UIX_LINE",
}


def require_tool(name: str) -> str:
    """解析必需工具，避免静默保留过期二进制。"""

    path = shutil.which(name)
    if path is None:
        raise SystemExit(f"缺少必需工具: {name}")
    return path


def compile_variant(
    glslc: str,
    validator: str,
    stage: str,
    output_name: str,
    macro: str,
    temporary: Path,
) -> None:
    """编译一个宏变体并在替换正式产物前完成 Vulkan 1.0 校验。"""

    source = SHADERS / f"rhi.{stage}"
    target = temporary / output_name
    combined = temporary / f"{output_name}.frag.combined"
    text = source.read_text()
    # 共享数学注入 #version 之后，保证跨变体单一定义。
    lines = text.split("\n")
    if stage == "frag":
        combined.write_text(
            "\n".join([lines[0], COMMON_SDF, *lines[1:]])
        )
        compile_source = combined
    else:
        compile_source = source
    subprocess.run(
        [
            glslc,
            "--target-env=vulkan1.0",
            "-O",
            f"-fshader-stage={stage}",
            f"-D{macro}=1",
            str(compile_source),
            "-o",
            str(target),
        ],
        check=True,
    )
    subprocess.run(
        [validator, "--target-env", "vulkan1.0", str(target)],
        check=True,
    )


def main() -> None:
    """原子生成全部变体，成功后才覆盖源码树中的提交产物。"""

    glslc = require_tool("glslc")
    validator = require_tool("spirv-val")
    OUTPUT.mkdir(parents=True, exist_ok=True)
    # 临时目录与正式产物位于同一文件系统，保证最终 replace 保持原子性。
    with tempfile.TemporaryDirectory(
        prefix="uix-vulkan-rhi-", dir=OUTPUT.parent
    ) as temp_dir:
        temporary = Path(temp_dir)
        for output_name, macro in VERTEX_VARIANTS.items():
            compile_variant(glslc, validator, "vert", output_name, macro, temporary)
        for output_name, macro in FRAGMENT_VARIANTS.items():
            compile_variant(glslc, validator, "frag", output_name, macro, temporary)
        for output_name in (*VERTEX_VARIANTS, *FRAGMENT_VARIANTS):
            (temporary / output_name).replace(OUTPUT / output_name)
    print(f"已生成并校验 {len(VERTEX_VARIANTS) + len(FRAGMENT_VARIANTS)} 个 SPIR-V 变体")


if __name__ == "__main__":
    main()
