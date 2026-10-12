"""Generate a CPU lifetime audit from the actual private presentation implementation.

Fake image storage/fences exercise selection and retirement, not Vulkan execution.
The pre-fix implementation is supported to demonstrate the failing media-sync paths.
No game data or renderer source is copied into the repository.
"""
import argparse
from pathlib import Path
import re


def block(text, start):
    opening = text.index("{", start)
    depth = 1
    end = opening + 1
    while depth:
        depth += (text[end] == "{") - (text[end] == "}")
        end += 1
    return text[opening:end]


def generate(root, baseline=False):
    directory = root if baseline else root / "rpcs3/Emu/RSX/VK"
    present = (directory / "VKPresent.cpp").read_text()
    header = (directory / "VKGSRender.h").read_text()
    upload = (directory / "VKTextureCache.cpp").read_text()
    modern = "struct present_source" in header
    marker = "VKGSRender::get_present_source("
    function = block(present, present.index(marker))
    if modern:
        owner = "struct present_source " + block(header, header.index("struct present_source")) + ";"
        retirement = block(present, present.index("for (auto& source : present_sources)"))
        retirement = "for (auto& source : present_sources) " + retirement
        retire_pos = present.index("for (auto& source : present_sources)")
        flip = present[present.index("void VKGSRender::flip("):]
        if retire_pos <= present.rindex("m_upscaler->scale_output("):
            raise ValueError("Source retirement precedes its final presentation use")
        if "flush_command_queue" in present[retire_pos:present.index("queue_swap_request();", retire_pos)]:
            raise ValueError("A sync occurs after retirement and before final submission")
        capture = flip[flip.index("if (g_cfg.video.record_with_overlays && has_overlay)"):]
        if "get_renderpass_key(image_to_flip->format())" not in capture.split("if (m_overlay_recording_img)")[0]:
            raise ValueError("Capture render pass does not match its attachment format")
        tail = block(upload, upload.index("texture_cache::upload_image_simple("))
        if "get_resource_manager()->dispose" in tail or not re.search(r"return image;\s*}", tail):
            raise ValueError("Linear upload does not return ownership to presentation")
        return_type = "VKGSRender::present_source"
        select = "return get_present_source(info, config);"
    else:
        owner = "struct present_source { vk::viewable_image* image{}; std::unique_ptr<vk::viewable_image> owned; bool reusable{}; };"
        retirement = ""
        return_type = "vk::viewable_image*"
        select = "present_source r; r.image = get_present_source(info, config); return r;"

    source = r'''
#include <algorithm>
#include <cstdint>
#include <iostream>
#include <memory>
#include <set>
#include <tuple>
#include <utility>
#include <vector>
using u32 = std::uint32_t;
using VkFormat = int;
constexpr int VK_IMAGE_TYPE_2D = 1, VK_IMAGE_USAGE_SAMPLED_BIT = 2;
constexpr int VK_IMAGE_USAGE_TRANSFER_SRC_BIT = 4, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL = 5;
constexpr int CELL_GCM_LOCATION_LOCAL = 0, RSX_FORMAT_CLASS_COLOR = 0;
struct areai { int x1, y1, x2, y2; };
namespace utils {
struct address_range32 { static int start_length(u32, u32) { return 0; } };
}
namespace rsx {
enum class surface_metrics { samples };
enum class surface_access { transfer_read };
enum class invalidation_cause { read };
struct avconf { int format = 1; };
namespace texture_upload_context { constexpr u32 blit_engine_dst = 1, framebuffer_storage = 2; }
int get_format_block_size_in_bytes(int) { return 4; }
template<bool> std::pair<u32, u32> apply_resolution_scale(int, u32 w, u32 h) { return {w, h}; }
}
int RSX_display_format_to_vk_format(int format) { return format; }
namespace vk {
std::set<const void*> live;
int recycled = 0, destroyed = 0, direct_copies = 0, typeless_copies = 0;
bool bitcast_compatible = false;
struct command_buffer { u32 flags{}; static constexpr u32 cb_has_dma_transfer = 1; };
struct viewable_image {
    int fmt; u32 w = 1280, h = 720;
    explicit viewable_image(int format = 1): fmt(format) { live.insert(this); }
    virtual ~viewable_image() { live.erase(this); ++destroyed; }
    int format() const { return fmt; }
    void change_layout(command_buffer&, int) {}
};
struct render_target : viewable_image {
    using viewable_image::viewable_image;
    template<rsx::surface_metrics> u32 get_surface_width() { return w; }
    template<rsx::surface_metrics> u32 get_surface_height() { return h; }
    viewable_image* get_surface(rsx::surface_access) { return this; }
};
render_target* as_rtt(render_target* r) { return r; }
struct present_surface_info { u32 address=0xc0001000, format=1, width=1280, height=720, pitch=5120; };
struct retirement { std::unique_ptr<viewable_image> data; bool reuse; };
struct fake_gc {
    std::vector<retirement> pending;
    std::vector<std::unique_ptr<viewable_image>> pool;
    void dispose(std::unique_ptr<viewable_image>& image) { pending.push_back({std::move(image), false}); }
    void recycle(std::unique_ptr<viewable_image>& image) { pending.push_back({std::move(image), true}); }
    void complete_submission() {
        for (auto& r : pending) {
            if (r.reuse) { ++recycled; pool.push_back(std::move(r.data)); }
            else r.data.reset();
        }
        pending.clear();
    }
} gc;
fake_gc* get_resource_manager() { return &gc; }
bool formats_are_bitcast_compatible(viewable_image*, viewable_image*) { return bitcast_compatible; }
void copy_image(command_buffer&, viewable_image*, viewable_image*, const areai&, const areai&) { ++direct_copies; }
void copy_image_typeless(command_buffer&, viewable_image*, viewable_image*, const areai&, const areai&) { ++typeless_copies; }
}
struct overlap { vk::render_target* surface; u32 base_address; };
struct rtt_store {
    vk::render_target image; bool available = false; u32 address = 0xc0001000;
    std::vector<overlap> get_merged_texture_memory_region(vk::command_buffer&, u32, u32, u32, u32, int, rsx::surface_access) {
        return available ? std::vector<overlap>{{&image, address}} : std::vector<overlap>{};
    }
};
struct cached_texture {
    vk::viewable_image image;
    u32 get_width() { return image.w; } u32 get_height() { return image.h; }
    vk::viewable_image* get_raw_texture() { return &image; }
    bool is_synchronized() { return true; }
    void copy_texture(vk::command_buffer&, bool) {}
};
struct texture_store {
    cached_texture cached; bool available = false, fail_upload = false, fail_conversion = false;
    template<bool> cached_texture* find_texture_from_dimensions(u32, u32) { return available ? &cached : nullptr; }
    template<bool> std::vector<cached_texture*> find_texture_from_range(int, int, u32) { return {}; }
    void invalidate_range(vk::command_buffer&, int, rsx::invalidation_cause) {}
    auto create_temporary_subresource_storage(int, int format, u32, u32, int, int, int, int, int, int) {
        return fail_conversion ? std::unique_ptr<vk::viewable_image>{} : std::make_unique<vk::viewable_image>(format);
    }
    void dispose_reusable_image(std::unique_ptr<vk::viewable_image>& image) { vk::gc.recycle(image); }
    UPLOAD_RETURN upload_image_simple(vk::command_buffer&, int format, u32, u32, u32, u32) {
        auto image = fail_upload ? std::unique_ptr<vk::viewable_image>{} : std::make_unique<vk::viewable_image>(format);
        UPLOAD_BODY
    }
};
struct VKGSRender {
    OWNER
    rtt_store m_rtts; texture_store m_texture_cache;
    vk::command_buffer cmd; vk::command_buffer* m_current_command_buffer = &cmd;
    int resolution_scaling_config = 100;
    RETURN_TYPE get_present_source(vk::present_surface_info*, const rsx::avconf&);
    void flush_command_queue() { vk::gc.complete_submission(); }
    present_source select(vk::present_surface_info* info, const rsx::avconf& config) { SELECT }
    void finish(present_source (&present_sources)[2]) { RETIRE }
};
RETURN_TYPE VKGSRender::get_present_source(vk::present_surface_info* info, const rsx::avconf& avconfig)
FUNCTION
int main() {
    int checks = 0, failures = 0;
    auto check = [&](const char* name, bool good) { ++checks; if(!good) { ++failures; std::cout << "FAIL " << name << '\n'; } };
    for(int mode = 0; mode < 7; ++mode) {
        vk::gc.complete_submission(); vk::gc.pool.clear();
        vk::bitcast_compatible = mode == 6;
        vk::direct_copies = vk::typeless_copies = 0;
        VKGSRender renderer;
        if(mode == 0 || mode == 3 || mode == 4 || mode == 6) renderer.m_rtts.available = true;
        if(mode == 1) renderer.m_texture_cache.available = true;
        if(mode == 3 || mode == 4 || mode == 6) renderer.m_rtts.image.fmt = 2;
        if(mode == 4) renderer.m_texture_cache.fail_conversion = true;
        if(mode == 5) renderer.m_texture_cache.fail_upload = true;
        vk::present_surface_info info; rsx::avconf config;
        VKGSRender::present_source sources[2];
        sources[0] = renderer.select(&info, config);
        sources[1] = renderer.select(&info, config);
        auto* left = sources[0].image; auto* right = sources[1].image;
        check("both eyes selected or upload rejected", mode == 5 ? !left && !right : left && right);
        if(mode == 3) check("typeless conversion retained", vk::typeless_copies == 2 && vk::direct_copies == 0);
        if(mode == 6) check("bitcast conversion retained", vk::direct_copies == 2 && vk::typeless_copies == 0);
        const int recycled_before = vk::recycled, destroyed_before = vk::destroyed;
        // Recording submits and waits for the upload/conversion commands. This
        // completion may run the same deferred retirements as the real renderer.
        vk::gc.complete_submission();
        check("media sync leaves left source alive", !left || vk::live.contains(left));
        check("media sync leaves right source alive", !right || vk::live.contains(right));
        check("media sync cannot recycle presentation source", vk::recycled == recycled_before);
        check("media sync cannot destroy presentation source", vk::destroyed == destroyed_before);
        // Record the final presentation use, queue retirement, then complete it.
        renderer.finish(sources);
        const bool owns = mode == 2 || mode == 3 || mode == 6;
        check("only owned sources retire at final use", vk::gc.pending.size() == (owns ? 2u : 0u));
        vk::gc.complete_submission();
        if(mode == 2) check("linear images destroyed after final submission", vk::destroyed == destroyed_before + 2);
        if(mode == 3 || mode == 6) check("conversion images recycled after final submission", vk::recycled == recycled_before + 2);
        if(mode == 0 || mode == 1 || mode == 4) check("borrowed sources preserved", vk::live.contains(left) && vk::live.contains(right));
    }
    std::cout << checks << " lifetime checks, " << failures << " failed\n";
    return failures ? 1 : 0;
}
'''
    values = {"OWNER": owner, "RETURN_TYPE": return_type, "SELECT": select,
              "RETIRE": retirement, "FUNCTION": function,
              "UPLOAD_RETURN": "std::unique_ptr<vk::viewable_image>" if modern else "vk::viewable_image*",
              "UPLOAD_BODY": "return image;" if modern else "auto* raw = image.get(); if(image) vk::gc.dispose(image); return raw;"}
    for key, value in values.items():
        source = source.replace(key, value)
    return source


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--checkout", type=Path, required=True)
    p.add_argument("--baseline", action="store_true", help="Read flat, exact pre-fix source backups")
    p.add_argument("--out", type=Path, required=True)
    args = p.parse_args()
    source = generate(args.checkout, args.baseline)
    args.out.mkdir(parents=True, exist_ok=False)
    path = args.out / "present-lifetime-audit.cpp"
    path.write_text(source, encoding="utf-8")
    print(path)


if __name__ == "__main__":
    main()
