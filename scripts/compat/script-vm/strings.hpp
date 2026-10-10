// Test-only lifetime and formatting observations from the actual native engine.
void refs(HSQUIRRELVM vm, std::string_view label) {
 SQObjectPtr temporary(SQString::Create(_ss(vm),"pool-shared"));
 std::cout<<"refs "<<label<<' '<<(_string(temporary)->_uiRef-1)<<'\n';
}
SQObjectPtr compile_file(HSQUIRRELVM vm, const char *filename) {
 std::ifstream input(filename,std::ios::binary);
 if(!input) throw std::runtime_error("missing source");
 std::string source((std::istreambuf_iterator<char>(input)),{});
 if(SQ_FAILED(sq_compilebuffer(vm,source,"fixture",SQFalse))) throw std::runtime_error("session compilation failed");
 SQObjectPtr closure=stack_get(vm,-1); sq_pop(vm,1); return closure;
}
void sliced(HSQUIRRELVM vm, SQObjectPtr &closure, SQObjectPtr &result) {
 const int credits[]={0,2,2,2,2,2,100};
 auto proto=_funcproto(_closure(closure)->_function);
 sq_pushroottable(vm);vm->_can_suspend=true;vm->_ops_till_suspend=credits[0];
 bool ok=vm->Call(closure,1,vm->_top-1,result,SQFalse,SQTrue);
 for(size_t i=0;;++i) {
  if(!ok) {std::cout<<"runtime_error "<<vm->_ops_till_suspend<<'\n';refs(vm,"error");break;}
  if(!vm->_suspended) {std::cout<<"return "<<vm->_ops_till_suspend<<' ';value(result);refs(vm,"returned");break;}
  std::cout<<"suspend "<<vm->_ops_till_suspend<<' '<<(vm->ci->_ip-proto->_instructions)<<'\n';refs(vm,"suspended");
  if(i+1>=std::size(credits)) throw std::runtime_error("unexpected nontermination");
  vm->_ops_till_suspend+=credits[i+1];
  ok=vm->Execute(_null_,vm->_top,-1,-1,result,SQFalse,SQVM::ET_RESUME_OPENTTD);
 }
 sq_settop(vm,0);
}
int realm_session(int argc, char **argv) {
 if(argc!=6)return 64;
 auto vm=sq_open(256);
 {
 refs(vm,"initial");
 auto first=compile_file(vm,argv[2]);refs(vm,"first");
 auto second=compile_file(vm,argv[3]);refs(vm,"second");
 auto dynamic=compile_file(vm,argv[4]);refs(vm,"dynamic");
 const auto &a=_funcproto(_closure(first)->_function)->_literals[0];
 const auto &b=_funcproto(_closure(second)->_function)->_literals[0];
 std::cout<<"same_literal "<<(_rawval(a)==_rawval(b))<<'\n';
 first.Null();refs(vm,"drop_first");
 auto runner=sq_newthread(vm,256); SQObjectPtr runner_owner=stack_get(vm,-1);sq_pop(vm,1);
 SQObjectPtr result;sliced(runner,dynamic,result);
 std::cout<<"same_runtime "<<(_rawval(result)==_rawval(b))<<'\n';
 runner_owner.Null();refs(vm,"drop_runner");
 second.Null();refs(vm,"drop_second");dynamic.Null();refs(vm,"drop_dynamic");
 std::cout<<"retained ";value(result);
 result.Null();refs(vm,"drop_result");
 auto error=compile_file(vm,argv[5]);sliced(vm,error,result);error.Null();refs(vm,"drop_error");
 }
 sq_close(vm);return 0;
}
int float_session(int argc,char **argv) {
 if(argc!=3)return 64;
 std::ifstream input(argv[2]);if(!input)return 66;
 auto vm=sq_open(256);
 { uint32_t bits;while(input>>bits) {
 SQObjectPtr source(std::bit_cast<float>(bits)),result;vm->ToString(source,result);
 std::cout<<bits<<' ';value(result);
 }}
 sq_close(vm);return 0;
}
int parallel_session(int argc, char **argv) {
 if(argc!=3)return 64;
 auto vm=sq_open(256);auto independent=sq_open(256);
 {
 auto program=compile_file(vm,argv[2]);auto other=compile_file(independent,argv[2]);
 auto literal=_funcproto(_closure(program)->_function)->_literals[0];
 auto foreign=_funcproto(_closure(other)->_function)->_literals[0];
 std::cout<<"separate_identity "<<(_rawval(literal)!=_rawval(foreign))<<'\n';
 literal.Null();foreign.Null();other.Null();
 refs(vm,"program");
 auto first=sq_newthread(vm,256);SQObjectPtr first_owner=stack_get(vm,-1);sq_pop(vm,1);
 auto second=sq_newthread(vm,256);SQObjectPtr second_owner=stack_get(vm,-1);sq_pop(vm,1);
 SQObjectPtr ignored,result;
 for(auto child : {first,second}) {
  sq_pushroottable(child);child->_can_suspend=true;child->_ops_till_suspend=2;
  if(!child->Call(program,1,child->_top-1,ignored,SQFalse,SQTrue)||!child->_suspended)throw std::runtime_error("expected load suspension");
  auto proto=_funcproto(_closure(program)->_function);
  std::cout<<"suspend "<<child->_ops_till_suspend<<' '<<(child->ci->_ip-proto->_instructions)<<'\n';
  refs(vm,"loaded");
 }
 first_owner.Null();refs(vm,"drop_first_runner");
 second->_ops_till_suspend+=100;
 if(!second->Execute(_null_,second->_top,-1,-1,result,SQFalse,SQVM::ET_RESUME_OPENTTD)||second->_suspended)throw std::runtime_error("expected return");
 std::cout<<"return "<<second->_ops_till_suspend<<' ';value(result);refs(vm,"returned");
 second_owner.Null();refs(vm,"drop_second_runner");
 program.Null();refs(vm,"drop_program");std::cout<<"retained ";value(result);
 result.Null();refs(vm,"drop_result");
 }
 sq_close(independent);sq_close(vm);return 0;
}
// These are the two original LoadFile feed callbacks, isolated from filesystem/BOM selection.
char32_t unsigned_feed(SQUserPointer data) {
 return static_cast<StringConsumer *>(data)->TryReadUint8().value_or(0);
}
char32_t utf8_feed(SQUserPointer data) {
 auto &input=*static_cast<StringConsumer *>(data);
 return input.AnyBytesLeft()?input.ReadUtf8(-1):0;
}
int feed_session(int argc,char **argv) {
 if(argc!=4)return 64;
 std::ifstream input(argv[3],std::ios::binary);if(!input)return 66;
 std::string source((std::istreambuf_iterator<char>(input)),{});StringConsumer reader(source);
 auto vm=sq_open(256);
 const std::string_view mode(argv[2]);
 auto callback=mode=="unsigned"?unsigned_feed:utf8_feed;
 if(mode!="unsigned"&&mode!="utf8")return 64;
 if(SQ_FAILED(sq_compile(vm,callback,&reader,"fixture",SQFalse)))std::cout<<"compile_error\n";
 else {
  auto proto=_funcproto(_closure(stack_get(vm,-1))->_function);
  std::cout<<"compiled\n";
  for(SQInteger i=0;i<proto->_nliterals;++i){std::cout<<"literal ";value(proto->_literals[i]);}
 }
 sq_close(vm);return 0;
}
int terminal_session(int argc,char **argv) {
 if(argc!=3)return 64;
 auto vm=sq_open(256);
 {
 auto closure=compile_file(vm,argv[2]);SQObjectPtr result;
 sliced(vm,closure,result);closure.Null();refs(vm,"drop_error_closure");
 }
 sq_close(vm);std::cout<<"closed\n";return 0;
}
int failed_compile_session(int argc,char **argv) {
 if(argc!=4)return 64;
 std::ifstream input(argv[2],std::ios::binary);if(!input)return 66;
 std::string source((std::istreambuf_iterator<char>(input)),{});
 auto vm=sq_open(256);refs(vm,"initial");
 if(SQ_SUCCEEDED(sq_compilebuffer(vm,source,"fixture",SQFalse)))throw std::runtime_error("expected failed compilation");
 std::cout<<"compile_error\n";refs(vm,"failed_compile");
 {auto good=compile_file(vm,argv[3]);refs(vm,"recompiled");}
 refs(vm,"released");sq_close(vm);return 0;
}
