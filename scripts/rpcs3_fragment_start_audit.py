"""Emit a standalone audit of the checked-out parser; does not modify RPCS3.

The v128 shim copies each 16-byte instruction exactly as the production SIMD
load does. Actual source functions and declarations are extracted rather than
reimplementing their parser logic. Compile the generated file as C++20.
"""

import argparse
from pathlib import Path
def definition(text, marker):
    start = text.index(marker)
    brace = text.index('{', start)
    depth = 1
    end = brace + 1
    while depth:
        depth += (text[end] == '{') - (text[end] == '}')
        end += 1
    return text[start:end]

def build_audit(root):
    source=(root/'rpcs3/Emu/RSX/Program/ProgramStateCache.cpp').read_text()
    header=(root/'rpcs3/Emu/RSX/Program/ProgramStateCache.h').read_text()
    ops=(root/'rpcs3/Emu/RSX/Program/Assembler/FPOpcodes.h').read_text()
    dest=(root/'rpcs3/Emu/RSX/Program/RSXFragmentProgram.h').read_text()
    parts=['''#include <array>
    #include <cstdint>
    #include <cstring>
    #include <iostream>
    using u32=uint32_t; using u64=uint64_t; using u16=uint16_t; using s32=int32_t;
    struct v128 {union {u32 _u32[4];u64 _u64[2];};
    static v128 loadu(const void* p,int index) {v128 v;std::memcpy(&v,static_cast<const char*>(p)+index*16,16);return v;}};
    namespace rsx::assembler {''', definition(ops,'enum FP_opcode')+';}',
        'using enum rsx::assembler::FP_opcode;', definition(dest,'union OPDEST')+';',
        'namespace program_hash_util {struct fragment_program_utils {',
        definition(header,'struct fragment_program_metadata')+';',
        'static bool is_any_src_constant(v128);static fragment_program_metadata analyse_fragment_program(const void*);};}',
        'using program_hash_util::fragment_program_utils;',
        definition(source,'bool fragment_program_utils::is_any_src_constant'),
        definition(source,'fragment_program_utils::fragment_program_metadata fragment_program_utils::analyse_fragment_program'),
        '''int main() {
    using Slot=std::array<u32,4>;
    auto inst=[](u32 opcode,bool end,int constant=-1){OPDEST d{};d.opcode=opcode;d.end=end;Slot s{OPDEST::from_be32(d.HEX).HEX,0,0,0};if(constant>=0)s[constant+1]=0x200;return s;};
    const Slot literal{0x9a99993f,0x6766e63e,0xb9187f3f,0x0000803f};
    int total=0,failed=0;
    auto test=[&](const char* name,const auto& slots,u32 start,u32 length,u32 constants=0,bool nop=false) {
    auto m=fragment_program_utils::analyse_fragment_program(slots.data());++total;
    if(m.program_start_offset!=start||m.program_ucode_length!=length||m.program_constants_buffer_length!=constants||m.is_nop_shader!=nop) {
    ++failed;std::cout<<"FAIL "<<name<<" start="<<m.program_start_offset<<" len="<<m.program_ucode_length<<" const="<<m.program_constants_buffer_length<<" nop="<<m.is_nop_shader<<'\\n';}
    };
    test("ordinary MOV",std::array{inst(1,true)},0,16);
    test("plain leading NOP",std::array{inst(0,false),inst(1,true)},16,16);
    for(int src=0;src<3;++src) {
    test("leading NOP literal",std::array{inst(0,false,src),literal,inst(1,true)},32,16);
    test("END NOP literal",std::array{inst(0,true,src),literal},0,32,16,true);
    }
    test("two NOP literals",std::array{inst(0,false,0),literal,inst(0,false,2),literal,inst(1,true)},64,16);
    test("ordinary MOV literal",std::array{inst(1,true,0),literal},0,32,16);
    test("END plain NOP",std::array{inst(0,true)},0,16,0,true);
    test("skip literal END bit",std::array{inst(0,false,0),literal,inst(2,true)},32,16);
    std::cout<<total<<" fixtures, "<<failed<<" failed\\n";return failed?1:0;
    }''']
    return '\n'.join(parts)


def main():
    parser = argparse.ArgumentParser(description='Generate C++ fixtures from the actual RPCS3 fragment parser.')
    parser.add_argument('--checkout', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True, help='new private output directory')
    args = parser.parse_args()
    audit = build_audit(args.checkout)
    args.out.mkdir(parents=True, exist_ok=False)
    path = args.out / 'fragment-start-audit.cpp'
    path.write_text(audit, encoding='utf-8')
    print(path)


if __name__ == '__main__':
    main()
