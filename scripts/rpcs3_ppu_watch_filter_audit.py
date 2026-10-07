"""Exercise the actual private PPU watch parser and entry callback with fake guest memory.

This checks trace filtering, budget retention and legacy watches. It does not
execute guest code, establish packet delivery or verify gameplay/graphics.
Generated C++ and the copied private header stay in the explicitly chosen output.
"""
import argparse
from pathlib import Path
import shutil

from rpcs3_present_lifetime_audit import block


def generate(root: Path) -> str:
    header = (root / "rpcs3/Emu/RSX/d1diag.h").read_text(encoding="utf-8")
    source = (root / "rpcs3/Emu/Cell/PPUThread.cpp").read_text(encoding="utf-8")
    watch = "struct ppu_watch " + block(header, header.index("struct ppu_watch")) + ";"
    poll = "void ppu_poll() " + block(header, header.index("inline void ppu_poll()"))
    hit = "void ppu_d1hit(ppu_thread& ppu, u64 addr, u64 r1) " + block(source, source.index("static void ppu_d1hit("))
    return r'''
#include <algorithm>
#include <array>
#include <cstdint>
#include <iostream>
#include <map>
#include <mutex>
#include <string>
#include <tuple>
#include <vector>
#include "ppu_watch_filter.h"
using u8=std::uint8_t; using u32=std::uint32_t; using u64=std::uint64_t; using usz=std::size_t;
constexpr auto umax=std::string::npos;
u64 get_system_time() { static u64 clock=0; return clock+=1000001; }
struct logger { unsigned calls=0; template<class... T> void warning(const char*,T&&...) { ++calls; } } ppu_log,d1_log;
namespace fmt {
template<class... T> std::string format(const char*,T&&...) { return "fixture"; }
template<class... T> void append(std::string&,const char*,T&&...) {}
}
namespace fs {
std::string control;
struct file { explicit file(std::string) {} explicit operator bool() const { return true; } std::string to_string() const { return control; } };
}
namespace vm {
constexpr int page_readable=1;
std::map<u32,u8> bytes;
unsigned checks=0,reads=0;
bool check_addr(u32 ptr,int,u32 size) {
    ++checks;
    if (u64{ptr}+size>(u64{1}<<32)) return false;
    for(u32 i=0;i<size;++i) if(!bytes.contains(ptr+i)) return false;
    return true;
}
u8 read8(u32 ptr) { ++reads; return bytes.at(ptr); }
u64 read64(u32 ptr) { u64 value=0; for(u32 i=0;i<8;++i) value=(value<<8)|read8(ptr+i); return value; }
}
struct ppu_thread { std::array<u64,32> gpr{}; u64 lr=0; const char* get_name() const { return "fixture"; } };
namespace d1diag {
std::array<u8,0x800000> mask{};
u8* const g_ppu_mask=mask.data();
std::mutex g_ppu_mutex;
WATCH
std::vector<ppu_watch> g_ppu_watches;
std::string ctl_dir() { return "fixture/"; }
POLL
}
HIT
unsigned checks=0,failures=0;
void check(const char* name,bool ok) { ++checks; if(!ok) ++failures; std::cout<<(ok?"PASS ":"FAIL ")<<name<<'\n'; }
void load(std::string text) { fs::control=std::move(text); d1diag::ppu_poll(); }
void set_be(u32 ptr,u32 width,u64 value) { for(u32 i=0;i<width;++i) vm::bytes[ptr+i]=u8(value>>((width-1-i)*8)); }
void enter(ppu_thread& ppu,u32 addr,u64 r1=0) { if(d1diag::g_ppu_mask[addr>>2]) ppu_d1hit(ppu,addr,r1); }
int main() {
    d1diag::ppu_watch_filter filter;
    check("register equality",d1diag::parse_ppu_watch_filter("if=r5==0x15",filter) && filter.reg==5 && filter.width==0 && filter.value==21);
    check("full 64-bit register value",d1diag::parse_ppu_watch_filter("if=r31==0xffffffffffffffff",filter) && filter.value==~u64{0});
    check("memory offset and big-endian width",d1diag::parse_ppu_watch_filter("if=m6+1:4==21",filter) && filter.reg==6 && filter.offset==1 && filter.width==4 && filter.value==21);
    check("memory default offset",d1diag::parse_ppu_watch_filter("if=m1:8==0xffffffffffffffff",filter) && filter.offset==0 && filter.width==8);
    check("maximum memory offset",d1diag::parse_ppu_watch_filter("if=m31+0xffffffff:1==255",filter) && filter.offset==0xffffffff);
    for(auto token: {"if=r0==21","if=r2==21","if=m0:4==21","if=m2:4==21",
                    "if=r32==21","if=r-1==21","if=r5==21tail","if=r5==+21","if=r5==0x",
                    "if=r5==18446744073709551616","if=r5+1==21","if=r5:4==21","if=m6==21",
                    "if=m6:3==21","if=m6:0==21","if=m6:1==256","if=m6+4294967296:4==21",
                    "if=m6+1:4==21==21","if=m6:4+1==21","if=m6+:4==21"}) {
        check(token,!d1diag::parse_ppu_watch_filter(token,filter));
    }
    ppu_thread ppu;
    load("100 2 u if=r5==21 r6:4\n");
    ppu.gpr[5]=17; ppu.gpr[6]=0x100; set_be(0x100,4,21);
    const auto logs=ppu_log.calls;
    for(unsigned i=0;i<100000;++i) enter(ppu,0x100);
    check("routine traffic cannot exhaust or disarm target watch",ppu_log.calls==logs && d1diag::g_ppu_watches[0].left==2 && d1diag::g_ppu_mask[0x100>>2]==1);
    check("rejected traffic does not populate unique keys",d1diag::g_ppu_watches[0].seen.empty());
    ppu.gpr[5]=21; enter(ppu,0x100);
    check("first target consumes one hit",ppu_log.calls==logs+1 && d1diag::g_ppu_watches[0].left==1 && d1diag::g_ppu_watches[0].seen.size()==1);
    enter(ppu,0x100);
    check("accepted duplicate preserves remaining budget",ppu_log.calls==logs+1 && d1diag::g_ppu_watches[0].left==1);
    set_be(0x100,4,22); enter(ppu,0x100);
    check("last distinct target logs and disarms",ppu_log.calls==logs+2 && d1diag::g_ppu_watches[0].left==0 && d1diag::g_ppu_mask[0x100>>2]==0);
    load("104 2 if=m6+1:4==21 if=r5==21\n");
    ppu.gpr[6]=0x200; set_be(0x201,4,21); ppu.gpr[5]=17;
    enter(ppu,0x104); check("all filters must match",d1diag::g_ppu_watches[0].left==2);
    ppu.gpr[5]=21; enter(ppu,0x104); check("unaligned big-endian subtype header matches",d1diag::g_ppu_watches[0].left==1);
    set_be(0x201,4,0x15000000); enter(ppu,0x104); check("little-endian interpretation is rejected",d1diag::g_ppu_watches[0].left==1);
    ppu.gpr[6]=0x300; const auto reads=vm::reads; enter(ppu,0x104);
    check("unreadable filter is skipped without reading or spending budget",vm::reads==reads && d1diag::g_ppu_watches[0].left==1 && d1diag::g_ppu_mask[0x104>>2]==1);
    ppu.gpr[6]=0xffffffff; const auto range_checks=vm::checks; enter(ppu,0x104);
    check("offset overflow cannot wrap to guest zero",vm::checks==range_checks && d1diag::g_ppu_watches[0].left==1);
    load("108 2 if=m6:4==21\n");
    ppu.gpr[6]=0xfffffffe; enter(ppu,0x108); check("memory width cannot wrap across guest address limit",d1diag::g_ppu_watches[0].left==2);
    ppu.gpr[6]=0x100000200ull; set_be(0x200,4,21); enter(ppu,0x108);
    check("guest pointers use low 32 bits",d1diag::g_ppu_watches[0].left==1);
    load("10c 2 if=r1==288 if=m1:1==7\n"); ppu.gpr[1]=0xdead; set_be(0x120,1,7); enter(ppu,0x10c,0x120);
    check("r1 filters use live translated-entry stack pointer",d1diag::g_ppu_watches[0].left==1);
    load("110 3 if=r5==21 if=m6:3==21\n114 2\n");
    check("invalid predicate rejects entire line",d1diag::g_ppu_watches.size()==1 && d1diag::g_ppu_watches[0].addr==0x114 && !d1diag::g_ppu_mask[0x110>>2]);
    enter(ppu,0x114); enter(ppu,0x114); check("ordinary watch retains existing hit budget",d1diag::g_ppu_watches[0].left==0 && !d1diag::g_ppu_mask[0x114>>2]);
    load("118 3 u r6:4\n"); ppu.gpr[6]=0x100; ppu.lr=1; set_be(0x100,4,7);
    enter(ppu,0x118); enter(ppu,0x118); ppu.lr=2; enter(ppu,0x118);
    check("ordinary unique watch still includes caller LR",d1diag::g_ppu_watches[0].left==1 && d1diag::g_ppu_watches[0].seen.size()==2);
    load("coverage\n"); const auto before=ppu_log.calls; enter(ppu,0x120); enter(ppu,0x120);
    check("coverage retains one-hit behavior",ppu_log.calls==before+1 && !d1diag::g_ppu_mask[0x120>>2]);
    load("# coverage disabled\n"); // Coverage remains requested until the literal is absent.
    load("# off\n"); check("coverage reload clears coverage masks",!d1diag::g_ppu_mask[0x124>>2]);
    d1diag::g_ppu_mask[0x128>>2]=1; enter(ppu,0x128); check("orphan mask is disarmed",!d1diag::g_ppu_mask[0x128>>2]);
    std::cout<<checks<<" PPU watch checks, "<<failures<<" failed\n";
    return failures?1:0;
}
'''.replace("WATCH", watch).replace("POLL", poll).replace("HIT", hit)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True, type=Path, help="Private RPCS3 source root")
    parser.add_argument("--out", required=True, type=Path, help="New private fixture directory")
    args = parser.parse_args()
    text = generate(args.root)
    args.out.mkdir(parents=True, exist_ok=False)
    shutil.copyfile(args.root / "rpcs3/Emu/RSX/Common/ppu_watch_filter.h", args.out / "ppu_watch_filter.h")
    output = args.out / "ppu-watch-filter-audit.cpp"
    output.write_text(text, encoding="utf-8")
    print(output)


if __name__ == "__main__":
    main()
