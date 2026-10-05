"""Generate standalone tests of the actual experimental depth-only predicate and parser."""
import argparse
from pathlib import Path
import re

from rpcs3_fragment_start_audit import build_audit


def generate(root):
    helper = (root / "rpcs3/Emu/RSX/Program/FragmentDepthOnly.h").read_text()
    enums = (root / "rpcs3/Emu/RSX/gcm_enums.h").read_text()
    effects = []
    for name in ("CELL_GCM_SHADER_CONTROL_DEPTH_EXPORT", "RSX_SHADER_CONTROL_USES_KIL"):
        match = re.search(name + r"\s*=\s*(0x[0-9a-fA-F]+)", enums)
        if not match:
            raise ValueError("Missing actual shader control declaration: " + name)
        effects.append(match[1])
    source = build_audit(root).replace("int main()", "int parser_main()")
    return source + "\n" + helper.replace("#pragma once", "") + "\n" + r'''
int main()
{
    using namespace rsx::fragment_depth_only;
    const std::uint32_t effects = EFFECTS;
    draw_state normal{};
    normal.control = 0x03000000;
    int total = 0, failed = 0;
    auto check = [&](const char* label, bool good) {
        ++total;
        if (!good) { ++failed; std::cout << "FAIL " << label << '\n'; }
    };
    check("eligible zero-local raster depth draw", eligible(normal, effects));
    for (int i = 0; i < 4; ++i) {
        auto state = normal; state.color_writes[i] = true;
        check("each color attachment excludes empty fragment", !eligible(state, effects));
    }
    auto state = normal; state.offset = 16;
    check("real shader address preserved", !eligible(state, effects));
    state = normal; state.location = 1;
    check("main-memory zero address preserved", !eligible(state, effects));
    state = normal; state.varying_mask = 1;
    check("fragment inputs preserved", !eligible(state, effects));
    for (std::uint32_t bit = 1; bit; bit <<= 1) {
        if (!(effects & bit)) continue;
        state = normal; state.control |= bit;
        check("each fragment side-effect bit excludes empty fragment", !eligible(state, effects));
    }
    state = normal; state.alpha_test = true;
    check("alpha test preserved", !eligible(state, effects));
    state = normal; state.alpha_to_coverage = true;
    check("alpha coverage preserved", !eligible(state, effects));
    state = normal; state.polygon_stipple = true;
    check("polygon discard preserved", !eligible(state, effects));
    state = normal; state.texture_alpha_kill = true;
    check("texture discard preserved", !eligible(state, effects));
    state = normal; state.color_writes[0] = true;
    check("same address becomes color draw", !eligible(state, effects));
    state.color_writes[0] = false;
    check("same address returns to raster depth draw", eligible(state, effects));
    auto code = end_nop();
    auto m = program_hash_util::fragment_program_utils::analyse_fragment_program(code.data());
    check("canonical END NOP is parsed at zero without literals or texture references",
          m.program_start_offset == 0 && m.program_ucode_length == 16 &&
          m.program_constants_buffer_length == 0 && m.referenced_textures_mask == 0 && m.is_nop_shader);
    auto copy = code;
    check("canonical storage is stable", copy == end_nop());
    const int parser_failed = parser_main();
    std::cout << total << " depth fixtures, " << failed << " failed\n";
    return failed || parser_failed ? 1 : 0;
}
'''.replace("EFFECTS", " | ".join(effects))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--checkout", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    args = p.parse_args()
    source = generate(args.checkout)
    args.out.mkdir(parents=True, exist_ok=False)
    path = args.out / "fragment-depth-audit.cpp"
    path.write_text(source, encoding="utf-8")
    print(path)


if __name__ == "__main__":
    main()
