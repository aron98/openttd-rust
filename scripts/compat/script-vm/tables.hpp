// Test-only passive observations of genuine bundled table objects and hashes.
#include <memory>
#include "src/3rdparty/squirrel/squirrel/sqtable.h"
static int table_hashes() {
 static_assert(sizeof(size_t)==8 && std::endian::native==std::endian::little);
#ifdef _LIBCPP_VERSION
 static_assert(_LIBCPP_ABI_VERSION==1);
 std::cerr<<"libc++ "<<_LIBCPP_VERSION<<" ABI "<<_LIBCPP_ABI_VERSION<<'\n';
#elif defined(__GLIBCXX__)
 std::cerr<<"libstdc++ "<<__GLIBCXX__<<'\n';
#else
#error unadmitted native standard library
#endif
 std::unique_ptr<SQVM,decltype(&sq_close)> owner(sq_open(256),sq_close);
 auto vm=owner.get();

 {
 for(size_t length=0;length<=4096;++length) {
  for(uint64_t seed=0;seed<8;++seed) {
   std::string bytes(length,'\0'); uint64_t state=seed+1;
   for(char &byte:bytes) { state^=state<<13;state^=state>>7;state^=state<<17;byte=char(state&255); }
   SQObjectPtr string=SQString::Create(_ss(vm),bytes);
   std::cout<<length<<' '<<seed<<' '<<_string(string)->_hash<<'\n';
  }
 }
 }
 return 0;
}
static int table_owner_session(int argc,char **argv) {
 if(argc!=3)return 64;
 std::ifstream input(argv[2],std::ios::binary);if(!input)return 66;
 std::string source((std::istreambuf_iterator<char>(input)),{});
 std::unique_ptr<SQVM,decltype(&sq_close)> owner(sq_open(256),sq_close);
 auto vm=owner.get();
 {
 auto run=[&](std::string_view code,SQObjectPtr &result) {
  if(SQ_FAILED(sq_compilebuffer(vm,code,"table-owner",SQFalse)))return false;
  SQObjectPtr closure=stack_get(vm,-1);sq_settop(vm,0);sq_pushroottable(vm);
  vm->_can_suspend=true;vm->_ops_till_suspend=10000;
  bool ok=vm->Call(closure,1,vm->_top-1,result,SQFalse,SQTrue);
  sq_settop(vm,0);return ok&&!vm->_suspended;
 };
 SQObjectPtr result;
 if(!run(source,result)||type(result)!=OT_TABLE){return 65;}
 SQObjectPtr watch=_table(result)->GetWeakRef(OT_TABLE);
 auto alive=[&](std::string_view stage){std::cout<<"alive "<<stage<<' '<<(type(_weakref(watch)->_obj)==OT_TABLE)<<'\n';};
 SQObjectPtr held=SQString::Create(_ss(vm),"held");
 SQObjectPtr key=SQString::Create(_ss(vm),"key");
 SQObjectPtr root_value;
 if(!_table(vm->_roottable)->Get(held,root_value))return 65;
 std::cout<<"same "<<(_table(root_value)==_table(result))<<'\n';root_value.Null();
 _table(result)->Set(key,SQObjectPtr(SQInteger(9)));
 result.Null();alive("result-dropped");
 _table(vm->_roottable)->Set(held,_null_);alive("root-cleared");
 if(SQ_FAILED(sq_compilebuffer(vm,"return 7;","next",SQFalse)))return 65;
 alive("compiled");sq_settop(vm,0);
 if(!run("return 7;",result))return 65;
 alive("temp-replaced");
 if(!run(source,result))return 65;
 SQObjectPtr second_watch=_table(result)->GetWeakRef(OT_TABLE);
 result.Null();
 if(!run("return held.key;",result))return 65;
 std::cout<<"persistent ";value(result);
 std::cout<<"alive root-only "<<(type(_weakref(second_watch)->_obj)==OT_TABLE)<<'\n';
 _table(vm->_roottable)->Set(held,_null_);
 std::cout<<"alive released "<<(type(_weakref(second_watch)->_obj)==OT_TABLE)<<'\n';
 }
 return 0;
}
static int table_mutation_session(int argc,char **argv) {
 if(argc!=3)return 64;
 std::ifstream input(argv[2],std::ios::binary);if(!input)return 66;
 std::string source((std::istreambuf_iterator<char>(input)),{});
 std::unique_ptr<SQVM,decltype(&sq_close)> owner(sq_open(256),sq_close);
 auto vm=owner.get();
 {
 SQObjectPtr table=SQTable::Create(_ss(vm),0);
 for(SQInteger key : {0,4,8,12})_table(table)->NewSlot(SQObjectPtr(key),SQObjectPtr(key));
 SQObjectPtr held=SQString::Create(_ss(vm),"held");
 _table(vm->_roottable)->NewSlot(held,table);
 if(SQ_FAILED(sq_compilebuffer(vm,source,"table-mutation",SQFalse)))return 65;
 SQObjectPtr closure=stack_get(vm,-1);sq_settop(vm,0);sq_pushroottable(vm);
 auto proto=_funcproto(_closure(closure)->_function);
 SQObjectPtr result;vm->_can_suspend=true;vm->_ops_till_suspend=2;
 bool ok=vm->Call(closure,1,vm->_top-1,result,SQFalse,SQTrue);
 for(int step=0;step<200;++step) {
  if(!ok)return 65;
  if(!vm->_suspended){std::cout<<"return "<<vm->_ops_till_suspend<<' ';value(result);break;}
  std::cout<<"suspend "<<vm->_ops_till_suspend<<' '<<(vm->ci->_ip-proto->_instructions)<<'\n';
  if(step==5){
   for(SQInteger key : {16,20,24,28,32})_table(table)->NewSlot(SQObjectPtr(key),SQObjectPtr(key+1));
   _table(vm->_roottable)->Set(held,SQObjectPtr(SQTable::Create(_ss(vm),0)));
   std::cout<<"host-mutated-and-rebound\n";
  }
  vm->_ops_till_suspend+=2;
  ok=vm->Execute(_null_,vm->_top,-1,-1,result,SQFalse,SQVM::ET_RESUME_OPENTTD);
 }
 }
 return 0;
}
static int table_pointer_keys() {
 std::unique_ptr<SQVM,decltype(&sq_close)> owner(sq_open(256),sq_close);auto vm=owner.get();
 SQObjectPtr table=SQTable::Create(_ss(vm),0);
 SQObjectPtr first=SQArray::Create(_ss(vm),0),second=SQTable::Create(_ss(vm),0),alias=first;
 _table(table)->NewSlot(first,SQObjectPtr(SQInteger(11)));
 _table(table)->NewSlot(second,SQObjectPtr(SQInteger(22)));
 SQObjectPtr found;
 std::cout<<"alias-lookup "<<_table(table)->Get(alias,found)<<' ';value(found);
 std::cout<<"hash first "<<HashObj(first)<<" second "<<HashObj(second)<<'\n';
 SQObjectPtr key,val;SQInteger cursor=0;
 while((cursor=_table(table)->Next(false,cursor,key,val))!=-1){std::cout<<"cursor "<<cursor<<' ';value(val);}
 return 0;
}
