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
#include "owner_trace.hpp"
void *sq_vm_malloc(SQUnsignedInteger n){ auto p=std::malloc(n); if(!p) std::abort(); return p; }
void *sq_vm_realloc(void *p,SQUnsignedInteger,SQUnsignedInteger n){auto q=std::realloc(p,n); if(!q && n) std::abort(); return q;}
void sq_vm_free(void *p,SQUnsignedInteger n){owner_trace::on_free(p,n);std::free(p);}
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
#include "arrays.hpp"
#include "tables.hpp"
#include "calls.hpp"
#include "closures.hpp"
#include "delegates.hpp"
static std::ofstream print_output;
static void record_print(HSQUIRRELVM, std::string_view bytes) {
 uint64_t size=bytes.size();
 for(unsigned shift=0;shift<64;shift+=8) print_output.put(static_cast<char>((size>>shift)&255));
 print_output.write(bytes.data(),static_cast<std::streamsize>(bytes.size()));
 print_output.flush();
 if(!print_output) std::exit(74);
}
int main(int argc, char **argv){
 if(argc < 2) return 64;
 if(std::string_view(argv[1])=="--delegate-owner") return delegate_owner_session(argc,argv);
 if(std::string_view(argv[1])=="--trap-error-owner") return trap_error_owner(argc,argv);
 if(std::string_view(argv[1])=="--closure-compound-store") return compound_store_errors();
 if(std::string_view(argv[1])=="--closure-publication") return closure_compile_publication();
 if(std::string_view(argv[1])=="--closure-persistence") return persistent_closure_session(false);
 if(std::string_view(argv[1])=="--closure-late-error") return persistent_closure_session(true);
 if(std::string_view(argv[1])=="--closure-acyclic") return lifetime(false);
 if(std::string_view(argv[1])=="--closure-cycle") return lifetime(true);
 if(std::string_view(argv[1])=="--closure-orphan") return failed_capture();
 if(std::string_view(argv[1])=="--closure-default-cycle") return default_cycle();
 if(std::string_view(argv[1])=="--closure-view") return prototype_view();
 const bool error_option=argc>2 && std::string_view(argv[2])=="--errors";
 const bool error_trace=error_option || std::getenv("OTTD_SCRIPT_ERROR_TRACE")!=nullptr;
 const bool closure_trace=std::getenv("OTTD_SCRIPT_CLOSURE_TRACE")!=nullptr;
 if(std::string_view(argv[1])=="--call-callee") return call_callee_owner();
 if(std::string_view(argv[1])=="--call-cycle-teardown") return call_cycle_teardown();
 if(std::string_view(argv[1])=="--call-failure") return call_failure_session(argc,argv);
 if(std::string_view(argv[1])=="--call-array") {
  if(argc!=3) return 64;
  if(std::string_view(argv[2])=="empty") return call_array_owners(false);
  if(std::string_view(argv[2])=="retained") return call_array_owners(true);
  return 64;
 }
 if(std::string_view(argv[1])=="--call-entry") {
  if(argc!=3) return 64;
  if(std::string_view(argv[2])=="empty") return call_entry_owners(false);
  if(std::string_view(argv[2])=="retained") return call_entry_owners(true);
  return 64;
 }
 if(std::string_view(argv[1])=="--root-teardown") return root_teardown_session();
 if(std::string_view(argv[1])=="--root-ownership") return root_ownership_session(argc,argv);
 if(std::string_view(argv[1])=="--root-registration") return root_registration_session();
 if(std::string_view(argv[1])=="--graph-teardown") return graph_teardown_session();
 if(std::string_view(argv[1])=="--graph-vm") return graph_vm_session(argc,argv);
 if(std::string_view(argv[1])=="--graph-storage") return graph_storage_session();
 if(std::string_view(argv[1])=="--table-effects") return table_effects_session(argc,argv);
 if(std::string_view(argv[1])=="--table-storage") return table_storage_session();
 if(std::string_view(argv[1])=="--table-removal") return table_mutation_session(argc,argv,true);
 if(std::string_view(argv[1])=="--table-pointer-keys") return table_pointer_keys();
 if(std::string_view(argv[1])=="--table-mutation") return table_mutation_session(argc,argv);
 if(std::string_view(argv[1])=="--table-owner") return table_owner_session(argc,argv);
 if(std::string_view(argv[1])=="--table-hashes") return table_hashes();
 if(std::string_view(argv[1])=="--array-session") return arrays_session(argc,argv);
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
 const int first_credit=(frames || error_option)?3:2;
 auto v=sq_open(256);
 if(const char *file=std::getenv("OTTD_SCRIPT_PRINT_FILE")) {
  print_output.open(file,std::ios::binary|std::ios::trunc);
  if(!print_output) return 74;
  sq_setprintfunc(v,record_print);
 }
 static_assert(sizeof(SQInteger)==8 && sizeof(SQFloat)==4);
#if defined(SQUSEDOUBLE) || defined(NO_GARBAGE_COLLECTOR)
#error wrong native variant
#endif
 if(SQ_FAILED(sq_compilebuffer(v,source,"fixture",SQFalse))) {
  std::cout<<"compile_error\n"; if(error_trace){std::cout<<"last_error ";value(v->_lasterror);} std::cerr<<_stringval(v->_lasterror)<<'\n'; sq_close(v); return 0;
 }
 // Retain closure while the call stack is mutated.
 {
 SQObjectPtr closure=stack_get(v,-1);
 auto proto=_funcproto(_closure(closure)->_function);
 if(closure_trace)function_metadata(proto);
 std::cout<<"stack "<<proto->_stacksize<<'\n';
 for(SQInteger n=0;n<proto->_nliterals;++n){std::cout<<"literal ";value(proto->_literals[n]);}
 for(SQInteger n=0;n<proto->_ninstructions;++n){const auto &i=proto->_instructions[n];std::cout<<"op "<<unsigned(i.op)<<' '<<unsigned(i._arg0)<<' '<<i._arg1<<' '<<unsigned(i._arg2)<<' '<<unsigned(i._arg3)<<'\n';}
 sq_pushroottable(v);
 SQObjectPtr result;
 v->_can_suspend=true;
 v->_ops_till_suspend=argc>first_credit?std::stoll(argv[first_credit]):10000;
 bool ok=v->Call(closure,1,v->_top-1,result,SQFalse,SQTrue);
 for(int step=0;;++step){
  if(error_trace){std::cout<<"last_error ";value(v->_lasterror);}
  if(!ok){std::cout<<"runtime_error "<<v->_ops_till_suspend<<'\n';if(type(v->_lasterror)==OT_STRING)std::cerr<<_stringval(v->_lasterror)<<'\n';break;}
  if(!v->_suspended){std::cout<<"return "<<v->_ops_till_suspend<<' ';value(result);break;}
  auto active=_funcproto(_closure(v->ci->_closure)->_function);
  if(closure_trace)std::cout<<"suspend "<<v->_ops_till_suspend<<' '<<function_path(proto,active)<<' '<<(v->ci->_ip-active->_instructions)<<' '<<v->_callsstacksize<<' '<<v->_stackbase<<' '<<v->_top<<'\n';
  else std::cout<<"suspend "<<v->_ops_till_suspend<<' '<<(v->ci->_ip-active->_instructions)<<'\n';
  if(frames) for(SQInteger slot=1;slot<active->_stacksize;++slot){
   std::cout<<"frame "<<slot<<' '; value(v->GetAt(v->_stackbase+slot));
  }
  if(step+first_credit+1>=argc) break;
  v->_ops_till_suspend+=std::stoll(argv[step+first_credit+1]);
  ok=v->Execute(_null_,v->_top,-1,-1,result,SQFalse,SQVM::ET_RESUME_OPENTTD);
 }
 }
 sq_close(v);
}
