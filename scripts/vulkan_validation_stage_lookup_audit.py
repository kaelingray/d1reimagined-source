"""Reproduce cross-stage variable-ID ambiguity using the actual GPU-AV lookup loop.

Supply the official SDK's post_process_descriptor_indexing.cpp. Generated C++ stays
private; this audit neither patches the layer nor validates any game shader.
"""
import argparse
from pathlib import Path

from rpcs3_present_lifetime_audit import block


def generate(source):
    text = source.read_text()
    start = text.index("const ::spirv::ResourceInterfaceVariable* resource_variable = nullptr;")
    loop = block(text, text.index("if (pipeline_state)", start))
    return r'''
#include <iostream>
#include <unordered_map>
#include <vector>
#define ASSERT_AND_CONTINUE(x) if (!(x)) continue
namespace spirv { struct ResourceInterfaceVariable { unsigned module, set; const char* type; }; }
struct EntryPoint { std::unordered_map<unsigned, const spirv::ResourceInterfaceVariable*> resource_interface_variable_map; };
struct ShaderStageState { const EntryPoint* entrypoint; };
struct Pipeline { std::vector<ShaderStageState> stage_states; };
struct Access { unsigned variable_id; };
const spirv::ResourceInterfaceVariable* lookup(const Pipeline* pipeline_state, Access descriptor_access) {
    const spirv::ResourceInterfaceVariable* resource_variable = nullptr;
LOOP
    return resource_variable;
}
int main() {
    spirv::ResourceInterfaceVariable vertex_buffer{1,0,"unsigned texel buffer"}, fragment_image{2,1,"float image"};
    EntryPoint vertex{{{17,&vertex_buffer}}}, fragment{{{17,&fragment_image}}};
    Pipeline stages{{{&vertex},{&fragment}}};
    int checks=0, failures=0;
    auto check=[&](const char* name,bool ok) { ++checks; if(!ok) { ++failures; std::cout<<"FAIL "<<name<<'\n'; } };
    // A fragment access has the same module-local variable ID as the vertex buffer.
    check("fragment access can receive vertex resource metadata", lookup(&stages,{17})==&vertex_buffer);
    stages.stage_states={{&fragment},{&vertex}};
    check("reversed stages can misassociate a vertex access", lookup(&stages,{17})==&fragment_image);
    fragment.resource_interface_variable_map={{18,&fragment_image}};
    stages.stage_states={{&vertex},{&fragment}};
    check("distinct module-local IDs avoid this counterexample", lookup(&stages,{18})==&fragment_image);
    check("unmatched ID returns no resource", lookup(&stages,{99})==nullptr);
    std::cout<<checks<<" stage-lookup checks, "<<failures<<" failed; "
             <<"two cross-stage counterexamples reproduced (no native causality claim)\n";
    return failures ? 1 : 0;
}
'''.replace("LOOP", loop)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    text = generate(args.source)
    args.out.mkdir(parents=True, exist_ok=False)
    path = args.out / "validation-stage-lookup-audit.cpp"
    path.write_text(text, encoding="utf-8")
    print(path)


if __name__ == "__main__":
    main()
