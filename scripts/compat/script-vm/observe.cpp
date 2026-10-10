// Test-only host plumbing; bundled compiler/VM sources remain pristine.
#include "src/stdafx.h"
#include <fstream>
#include "src/core/string_consumer.hpp"
#include <iostream>
#include "src/3rdparty/squirrel/squirrel/sqpcheader.h"
#include "src/3rdparty/squirrel/squirrel/sqvm.h"
#include "src/3rdparty/squirrel/squirrel/sqfuncproto.h"
#include "src/3rdparty/squirrel/squirrel/sqclosure.h"
#include "src/3rdparty/squirrel/squirrel/sqstring.h"
void *sq_vm_malloc(SQUnsignedInteger n){ auto p=std::malloc(n); if(!p) std::abort(); return p; }
void *sq_vm_realloc(void *p,SQUnsignedInteger,SQUnsignedInteger n){auto q=std::realloc(p,n); if(!q && n) std::abort(); return q;}
void sq_vm_free(void *p,SQUnsignedInteger){std::free(p);}
[[noreturn]] void AssertFailedError(std::string_view msg, std::source_location){std::cerr<<msg; std::abort();}
[[noreturn]] void NOT_REACHED(std::source_location){std::abort();}
void DebugPrint(std::string_view, int, std::string &&msg){std::cerr<<msg<<'\n';}
void value(const SQObjectPtr &v) {
 switch(type(v)) {
 case OT_NULL: std::cout<<"null"; break;
 case OT_INTEGER: std::cout<<"integer "<<_integer(v); break;
 case OT_FLOAT: std::cout<<"float "<<std::bit_cast<uint32_t>(_float(v)); break;
 case OT_BOOL: std::cout<<"bool "<<_integer(v); break;
 case OT_STRING: {
  const auto bytes=_stringval(v); std::cout<<"string "<<bytes.size()<<' ';
  static constexpr char hex[]="0123456789abcdef";
  for(unsigned char byte : bytes) std::cout<<hex[byte >> 4]<<hex[byte & 15];
  break;
 }
 default: std::cout<<"unsupported "<<type(v); break;
 }
 std::cout<<'\n';
}
#include "strings.hpp"
#include "constants.hpp"
#include "root_slots.hpp"
int main(int argc, char **argv){
 if(argc < 2) return 64;
 if(std::string_view(argv[1])=="--root-slots-session") return root_slots_session(argc,argv);
 if(std::string_view(argv[1])=="--constants-session") return constants_session(argc,argv);
 if(std::string_view(argv[1])=="--parallel") return parallel_session(argc,argv);
 if(std::string_view(argv[1])=="--feed") return feed_session(argc,argv);
 if(std::string_view(argv[1])=="--terminal") return terminal_session(argc,argv);
 if(std::string_view(argv[1])=="--compile-failure") return failed_compile_session(argc,argv);
 if(std::string_view(argv[1])=="--realm") return realm_session(argc,argv);
 if(std::string_view(argv[1])=="--format") return float_session(argc,argv);
 std::ifstream input(argv[1],std::ios::binary); if(!input) return 66;
 std::string source((std::istreambuf_iterator<char>(input)),{});
 const bool frames=argc>2 && std::string_view(argv[2])=="--frames";
 const int first_credit=frames?3:2;
 auto v=sq_open(256);
 static_assert(sizeof(SQInteger)==8 && sizeof(SQFloat)==4);
#if defined(SQUSEDOUBLE) || defined(NO_GARBAGE_COLLECTOR)
#error wrong native variant
#endif
 if(SQ_FAILED(sq_compilebuffer(v,source,"fixture",SQFalse))) {
  std::cout<<"compile_error\n"; std::cerr<<_stringval(v->_lasterror)<<'\n'; sq_close(v); return 0;
 }
 // Retain closure while the call stack is mutated.
 {
 SQObjectPtr closure=stack_get(v,-1);
 auto proto=_funcproto(_closure(closure)->_function);
 std::cout<<"stack "<<proto->_stacksize<<'\n';
 for(SQInteger n=0;n<proto->_nliterals;++n){std::cout<<"literal ";value(proto->_literals[n]);}
 for(SQInteger n=0;n<proto->_ninstructions;++n){const auto &i=proto->_instructions[n];std::cout<<"op "<<unsigned(i.op)<<' '<<unsigned(i._arg0)<<' '<<i._arg1<<' '<<unsigned(i._arg2)<<' '<<unsigned(i._arg3)<<'\n';}
 sq_pushroottable(v);
 SQObjectPtr result;
 v->_can_suspend=true;
 v->_ops_till_suspend=argc>first_credit?std::stoll(argv[first_credit]):10000;
 bool ok=v->Call(closure,1,v->_top-1,result,SQFalse,SQTrue);
 for(int step=0;;++step){
  if(!ok){std::cout<<"runtime_error "<<v->_ops_till_suspend<<'\n';std::cerr<<_stringval(v->_lasterror)<<'\n';break;}
  if(!v->_suspended){std::cout<<"return "<<v->_ops_till_suspend<<' ';value(result);break;}
  std::cout<<"suspend "<<v->_ops_till_suspend<<' '<<(v->ci->_ip-proto->_instructions)<<'\n';
  if(frames) for(SQInteger slot=1;slot<proto->_stacksize;++slot){
   std::cout<<"frame "<<slot<<' '; value(v->GetAt(v->_stackbase+slot));
  }
  if(step+first_credit+1>=argc) break;
  v->_ops_till_suspend+=std::stoll(argv[step+first_credit+1]);
  ok=v->Execute(_null_,v->_top,-1,-1,result,SQFalse,SQVM::ET_RESUME_OPENTTD);
 }
 }
 sq_close(v);
}
