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
static int table_mutation_session(int argc,char **argv,bool remove=false) {
 if(argc!=3)return 64;
 std::ifstream input(argv[2],std::ios::binary);if(!input)return 66;
 std::string source((std::istreambuf_iterator<char>(input)),{});
 std::unique_ptr<SQVM,decltype(&sq_close)> owner(sq_open(256),sq_close);
 auto vm=owner.get();
 {
 SQObjectPtr table=SQTable::Create(_ss(vm),0);
 if(remove){for(SQInteger key=0;key<32;++key)_table(table)->NewSlot(SQObjectPtr(key),SQObjectPtr(key));}
 else for(SQInteger key : {0,4,8,12})_table(table)->NewSlot(SQObjectPtr(key),SQObjectPtr(key));
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
   if(remove){for(SQInteger key=2;key<32;++key)_table(table)->Remove(SQObjectPtr(key));}
   else for(SQInteger key : {16,20,24,28,32})_table(table)->NewSlot(SQObjectPtr(key),SQObjectPtr(key+1));
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

static int table_storage_session() {
 std::unique_ptr<SQVM,decltype(&sq_close)> owner(sq_open(256),sq_close);auto vm=owner.get();
 {
 SQObjectPtr source=SQTable::Create(_ss(vm),0);
 SQObjectPtr hidden=SQString::Create(_ss(vm),"hidden-value");
 SQObjectPtr watch=_string(hidden)->GetWeakRef(OT_STRING);
 _table(source)->Set(_null_,hidden);hidden.Null();
 SQObjectPtr copy=_table(source)->Clone();
 std::cout<<"counts "<<_table(source)->CountUsed()<<' '<<_table(copy)->CountUsed()<<'\n';
 std::cout<<"hidden before "<<(type(_weakref(watch)->_obj)==OT_STRING)<<'\n';
 source.Null();
 std::cout<<"hidden after-source "<<(type(_weakref(watch)->_obj)==OT_STRING)<<'\n';
 SQObjectPtr key=SQString::Create(_ss(vm),"key"),value=SQString::Create(_ss(vm),"removed-value");
 watch=_string(value)->GetWeakRef(OT_STRING);
 _table(copy)->NewSlot(key,value);value.Null();
 SQObjectPtr removed; if(!vm->DeleteSlot(copy,key,removed))return 65;
 std::cout<<"removed count "<<_table(copy)->CountUsed()<<'\n';
 std::cout<<"removed held "<<(type(_weakref(watch)->_obj)==OT_STRING)<<'\n';
 removed.Null();std::cout<<"removed released "<<(type(_weakref(watch)->_obj)==OT_STRING)<<'\n';
 SQObjectPtr array=SQArray::Create(_ss(vm),0);
 value=SQString::Create(_ss(vm),"array-value");watch=_string(value)->GetWeakRef(OT_STRING);
 _array(array)->Append(value);value.Null();
 SQObjectPtr array_copy=_array(array)->Clone();
 std::cout<<"array distinct "<<(_array(array)!=_array(array_copy))<<'\n';
 array.Null();std::cout<<"array source-dropped "<<(type(_weakref(watch)->_obj)==OT_STRING)<<'\n';
 array_copy.Null();std::cout<<"array copy-dropped "<<(type(_weakref(watch)->_obj)==OT_STRING)<<'\n';
 }
 return 0;
}

static int table_effects_session(int argc,char **argv) {
 if(argc!=3)return 64;
 std::ifstream input(argv[2],std::ios::binary);if(!input)return 66;
 std::string source((std::istreambuf_iterator<char>(input)),{});
 std::unique_ptr<SQVM,decltype(&sq_close)> owner(sq_open(256),sq_close);auto vm=owner.get();
 {
 if(SQ_FAILED(sq_compilebuffer(vm,source,"table-effects",SQFalse)))return 65;
 SQObjectPtr closure=stack_get(vm,-1);sq_settop(vm,0);sq_pushroottable(vm);
 SQObjectPtr result;vm->_can_suspend=true;vm->_ops_till_suspend=10000;
 bool ok=vm->Call(closure,1,vm->_top-1,result,SQFalse,SQTrue);
 std::cout<<"result "<<ok<<' '<<vm->_ops_till_suspend<<'\n';
 for(std::string_view name : {"held","copy"}) {
  SQObjectPtr table;
  if(!_table(vm->_roottable)->Get(SQObjectPtr(SQString::Create(_ss(vm),name)),table)||type(table)!=OT_TABLE)return 65;
  std::cout<<name<<" count "<<_table(table)->CountUsed()<<'\n';
  SQObjectPtr key,val;SQInteger cursor=0;
  while((cursor=_table(table)->Next(false,cursor,key,val))!=-1){std::cout<<"cursor "<<cursor<<' ';value(key);value(val);}
 }
 }
 return 0;
}

// Host handles registered with sq_addref are GC roots, unlike bare local SQObjectPtr.
static int graph_storage_session() {
 std::unique_ptr<SQVM,decltype(&sq_close)> owner(sq_open(256),sq_close);auto vm=owner.get();
 auto alive=[](const SQObjectPtr &watch){return type(_weakref(watch)->_obj)!=OT_NULL;};
 {
 SQObjectPtr key=SQString::Create(_ss(vm),"key");
 SQObjectPtr leaf=SQTable::Create(_ss(vm),0);
 _table(leaf)->NewSlot(key,SQObjectPtr(SQString::Create(_ss(vm),"graph-leaf")));
 SQObjectPtr watch=_table(leaf)->GetWeakRef(OT_TABLE);
 SQObjectPtr array=SQArray::Create(_ss(vm),0);_array(array)->Append(leaf);
 SQObjectPtr parent=SQTable::Create(_ss(vm),0);_table(parent)->NewSlot(key,array);
 SQObjectPtr copy=_table(parent)->Clone(),found;
 _table(copy)->Get(key,found);
 std::cout<<"clone alias "<<(_array(found)==_array(array))<<'\n';found.Null();
 leaf.Null();array.Null();parent.Null();
 std::cout<<"dag clone-held "<<alive(watch)<<'\n';copy.Null();
 std::cout<<"dag released "<<alive(watch)<<'\n';
 SQObjectPtr hidden=SQTable::Create(_ss(vm),0);
 leaf=SQTable::Create(_ss(vm),0);watch=_table(leaf)->GetWeakRef(OT_TABLE);
 _table(hidden)->Set(_null_,leaf);leaf.Null();copy=_table(hidden)->Clone();
 std::cout<<"hidden held "<<alive(watch)<<'\n';hidden.Null();
 std::cout<<"hidden clone-omits "<<alive(watch)<<'\n';copy.Null();
 SQObjectPtr cycle=SQTable::Create(_ss(vm),0);
 watch=_table(cycle)->GetWeakRef(OT_TABLE);_table(cycle)->NewSlot(key,cycle);
 sq_addref(vm,&cycle);
 std::cout<<"cycle rooted-collect "<<sq_collectgarbage(vm)<<' '<<alive(watch)<<'\n';
 sq_release(vm,&cycle);cycle.Null();
 std::cout<<"cycle unrooted-before "<<alive(watch)<<'\n';
 std::cout<<"cycle collected "<<sq_collectgarbage(vm)<<' '<<alive(watch)<<'\n';
 array=SQArray::Create(_ss(vm),0);parent=SQTable::Create(_ss(vm),0);
 SQObjectPtr array_watch=_array(array)->GetWeakRef(OT_ARRAY);
 watch=_table(parent)->GetWeakRef(OT_TABLE);
 _array(array)->Append(parent);_table(parent)->NewSlot(key,array);
 sq_addref(vm,&array);parent.Null();
 std::cout<<"pair rooted-collect "<<sq_collectgarbage(vm)<<' '<<alive(watch)<<' '<<alive(array_watch)<<'\n';
 sq_release(vm,&array);array.Null();
 std::cout<<"pair unrooted-before "<<alive(watch)<<' '<<alive(array_watch)<<'\n';
 std::cout<<"pair collected "<<sq_collectgarbage(vm)<<' '<<alive(watch)<<' '<<alive(array_watch)<<'\n';
 leaf=SQTable::Create(_ss(vm),0);watch=_table(leaf)->GetWeakRef(OT_TABLE);
 SQObjectPtr chain=leaf;leaf.Null();
 for(int index=0;index<2048;++index){SQObjectPtr next=SQArray::Create(_ss(vm),0);_array(next)->Append(chain);chain=next;}
 std::cout<<"deep held "<<alive(watch)<<'\n';chain.Null();
 std::cout<<"deep released "<<alive(watch)<<'\n';
 }
 return 0;
}

static int graph_vm_session(int argc,char **argv) {
 if(argc!=4)return 64;
 auto read=[](const char *path){std::ifstream input(path,std::ios::binary);if(!input)throw std::runtime_error("missing graph fixture");return std::string((std::istreambuf_iterator<char>(input)),{});};
 const auto suspended=read(argv[2]),failure=read(argv[3]);
 std::unique_ptr<SQVM,decltype(&sq_close)> owner(sq_open(256),sq_close);auto vm=owner.get();
 auto alive=[](const SQObjectPtr &watch){return type(_weakref(watch)->_obj)!=OT_NULL;};
 {
 SQObjectPtr held=SQString::Create(_ss(vm),"held"),key=SQString::Create(_ss(vm),"key");
 SQObjectPtr leaf=SQTable::Create(_ss(vm),0),array=SQArray::Create(_ss(vm),0);
 _table(leaf)->NewSlot(key,SQObjectPtr(SQInteger(7)));_array(array)->Append(leaf);
 _table(vm->_roottable)->NewSlot(held,array);
 SQObjectPtr leaf_watch=_table(leaf)->GetWeakRef(OT_TABLE),array_watch=_array(array)->GetWeakRef(OT_ARRAY);
 if(SQ_FAILED(sq_compilebuffer(vm,suspended,"graph",SQFalse)))return 65;
 SQObjectPtr closure=stack_get(vm,-1);sq_settop(vm,0);sq_pushroottable(vm);
 SQObjectPtr result;vm->_can_suspend=true;vm->_ops_till_suspend=2;
 bool ok=vm->Call(closure,1,vm->_top-1,result,SQFalse,SQTrue);
 if(!ok||!vm->_suspended)return 65;
 std::cout<<"suspended "<<vm->_ops_till_suspend<<' '<<(vm->ci->_ip-_funcproto(_closure(closure)->_function)->_instructions)<<'\n';
 _table(vm->_roottable)->Set(held,_null_);leaf.Null();array.Null();sq_collectgarbage(vm);
 std::cout<<"frame-held "<<alive(array_watch)<<' '<<alive(leaf_watch)<<'\n';
 vm->_ops_till_suspend+=100;
 ok=vm->Execute(_null_,vm->_top,-1,-1,result,SQFalse,SQVM::ET_RESUME_OPENTTD);
 if(!ok||vm->_suspended||type(result)!=OT_ARRAY)return 65;
 sq_addref(vm,&result);sq_settop(vm,0);closure.Null();
 std::cout<<"returned "<<vm->_ops_till_suspend<<'\n';
 auto run=[&](std::string_view source) {
  if(SQ_FAILED(sq_compilebuffer(vm,source,"graph",SQFalse)))throw std::runtime_error("compile graph");
  SQObjectPtr fn=stack_get(vm,-1);sq_settop(vm,0);sq_pushroottable(vm);SQObjectPtr scalar;
  vm->_ops_till_suspend=10000;
  bool passed=vm->Call(fn,1,vm->_top-1,scalar,SQFalse,SQTrue);sq_settop(vm,0);return passed;
 };
 if(!run("return 1;"))return 65;sq_collectgarbage(vm);
 std::cout<<"host-result-held "<<alive(array_watch)<<' '<<alive(leaf_watch)<<'\n';
 sq_release(vm,&result);result.Null();
 std::cout<<"result-released "<<alive(array_watch)<<' '<<alive(leaf_watch)<<'\n';
 bool passed=run(failure);std::cout<<"late-error "<<passed<<' '<<vm->_ops_till_suspend<<'\n';
 if(!_table(vm->_roottable)->Get(held,array)||type(array)!=OT_ARRAY)return 65;
 if(!_array(array)->Get(0,leaf)||type(leaf)!=OT_TABLE)return 65;
 array_watch=_array(array)->GetWeakRef(OT_ARRAY);leaf_watch=_table(leaf)->GetWeakRef(OT_TABLE);
 SQObjectPtr scalar;_table(leaf)->Get(key,scalar);std::cout<<"late-value ";value(scalar);
 array.Null();leaf.Null();sq_collectgarbage(vm);
 std::cout<<"late-root-held "<<alive(array_watch)<<' '<<alive(leaf_watch)<<'\n';
 _table(vm->_roottable)->Set(held,_null_);
 std::cout<<"late-root-cleared "<<alive(array_watch)<<' '<<alive(leaf_watch)<<'\n';
 if(!run("return 1;"))return 65;
 std::cout<<"late-temp-replaced "<<alive(array_watch)<<' '<<alive(leaf_watch)<<'\n';
 }
 return 0;
}

static int graph_teardown_session() {
 // SQWeakRef is not shared-state-owned; finalized objects null its payload.
 SQObjectPtr table_watch,array_watch;
 std::unique_ptr<SQVM,decltype(&sq_close)> owner(sq_open(256),sq_close);auto vm=owner.get();
 {
 SQObjectPtr table=SQTable::Create(_ss(vm),0),array=SQArray::Create(_ss(vm),0);
 table_watch=_table(table)->GetWeakRef(OT_TABLE);array_watch=_array(array)->GetWeakRef(OT_ARRAY);
 _array(array)->Append(table);_table(table)->NewSlot(SQObjectPtr(SQInteger(1)),array);
 }
 std::cout<<"teardown before "<<(type(_weakref(table_watch)->_obj)==OT_TABLE)<<' '<<(type(_weakref(array_watch)->_obj)==OT_ARRAY)<<'\n';
 owner.reset();
 std::cout<<"teardown after "<<(type(_weakref(table_watch)->_obj)==OT_TABLE)<<' '<<(type(_weakref(array_watch)->_obj)==OT_ARRAY)<<'\n';
 return 0;
}
// Passive registration/identity observations from the unmodified original VM.
static int root_registration_session() {
 auto owner=std::unique_ptr<SQVM,decltype(&sq_close)>(sq_open(256),sq_close);
 auto vm=owner.get(); SQObjectPtr root=vm->_roottable;
 SQObjectPtr cursor,key,slot;
 while(true) {
  SQInteger next=_table(root)->Next(false,cursor,key,slot); if(next==-1)break; cursor=next;
  std::cout<<"slot "<<next<<' '<<_stringval(key)<<' ';value(slot);
  if(type(slot)==OT_NATIVECLOSURE) {
   auto closure=_nativeclosure(slot);
   std::cout<<"metadata "<<_stringval(closure->_name)<<' '<<closure->_nparamscheck<<' '<<closure->_outervalues.size()<<' '<<type(closure->_env);
   for(SQUnsignedInteger i=0;i<closure->_typecheck.size();++i)std::cout<<' '<<closure->_typecheck[i];
   std::cout<<'\n';
  }
 }
 slot.Null();key.Null();
 SQObjectPtr name=SQString::Create(_ss(vm),"print"),old;
 _table(root)->Get(name,old);SQObjectPtr watch=_nativeclosure(old)->GetWeakRef(OT_NATIVECLOSURE);
 SQObjectPtr copy=_table(root)->Clone(),copied;_table(copy)->Get(name,copied);
 std::cout<<"clone alias "<<(_nativeclosure(old)==_nativeclosure(copied))<<'\n';copied.Null();
 auto friendvm=sq_newthread(vm,256);if(!friendvm)return 74;
 SQObjectPtr current;_table(root)->Get(name,current);
 std::cout<<"friend root "<<(_table(root)==_table(friendvm->_roottable))<<'\n';
 std::cout<<"friend replaced "<<(_nativeclosure(old)!=_nativeclosure(current))<<'\n';
 old.Null();std::cout<<"old clone-held "<<(type(_weakref(watch)->_obj)==OT_NATIVECLOSURE)<<'\n';
 copy.Null();std::cout<<"old released "<<(type(_weakref(watch)->_obj)==OT_NATIVECLOSURE)<<'\n';
 SQObjectPtr current_watch=_nativeclosure(current)->GetWeakRef(OT_NATIVECLOSURE);
 current.Null();sq_pop(vm,1);std::cout<<"current root-held "<<(type(_weakref(current_watch)->_obj)==OT_NATIVECLOSURE)<<'\n';
 _table(root)->Remove(name);std::cout<<"current deleted "<<(type(_weakref(current_watch)->_obj)==OT_NATIVECLOSURE)<<'\n';
 return 0;
}

static int root_ownership_session(int argc,char **argv) {
 if(argc!=3&&argc!=4)return 64;
 bool reverse=argc==4&&std::string_view(argv[3])=="reverse";
 std::ifstream input(argv[2],std::ios::binary);if(!input)return 66;
 const std::string source((std::istreambuf_iterator<char>(input)),{});
 std::unique_ptr<SQVM,decltype(&sq_close)> owner(sq_open(256),sq_close);auto vm=owner.get();
 auto alive=[](const SQObjectPtr &watch){return type(_weakref(watch)->_obj)!=OT_NULL;};
 SQObjectPtr root_watch=_table(vm->_roottable)->GetWeakRef(OT_TABLE),print;
 _table(vm->_roottable)->Get(SQObjectPtr(SQString::Create(_ss(vm),"print")),print);
 SQObjectPtr print_watch=_nativeclosure(print)->GetWeakRef(OT_NATIVECLOSURE);print.Null();
 if(SQ_FAILED(sq_compilebuffer(vm,source,"roots",SQFalse)))return 65;
 SQObjectPtr fn=stack_get(vm,-1);sq_settop(vm,0);sq_pushroottable(vm);
 SQObjectPtr result;vm->_can_suspend=true;vm->_ops_till_suspend=2;
 if(!vm->Call(fn,1,vm->_top-1,result,SQFalse,SQTrue)||!vm->_suspended)return 65;
 std::cout<<"suspended "<<vm->_ops_till_suspend<<' '<<(vm->ci->_ip-_funcproto(_closure(fn)->_function)->_instructions)<<'\n';
 vm->_roottable=SQTable::Create(_ss(vm),0);
 std::cout<<"frame collect "<<sq_collectgarbage(vm)<<' '<<alive(root_watch)<<' '<<alive(print_watch)<<'\n';
 vm->_ops_till_suspend+=100;
 if(!vm->Execute(_null_,vm->_top,-1,-1,result,SQFalse,SQVM::ET_RESUME_OPENTTD)||vm->_suspended||type(result)!=OT_ARRAY)return 65;
 sq_addref(vm,&result);sq_settop(vm,0);fn.Null();
 std::cout<<"returned "<<vm->_ops_till_suspend<<'\n';
 SQObjectPtr original,copy,self,left,right;
 _array(result)->Get(reverse?1:0,original);_array(result)->Get(reverse?0:1,copy);
 _table(copy)->Get(SQObjectPtr(SQString::Create(_ss(vm),"self")),self);
 _table(original)->Get(SQObjectPtr(SQString::Create(_ss(vm),"print")),left);
 _table(copy)->Get(SQObjectPtr(SQString::Create(_ss(vm),"print")),right);
 std::cout<<"escaped aliases "<<(_table(original)!=_table(copy))<<' '<<(_table(self)==_table(original))<<' '<<(_nativeclosure(left)==_nativeclosure(right))<<'\n';
 original.Null();copy.Null();self.Null();left.Null();right.Null();
 if(SQ_FAILED(sq_compilebuffer(vm,"return 1;","roots",SQFalse)))return 65;
 fn=stack_get(vm,-1);sq_settop(vm,0);sq_pushroottable(vm);SQObjectPtr scalar;vm->_ops_till_suspend=10000;
 if(!vm->Call(fn,1,vm->_top-1,scalar,SQFalse,SQTrue))return 65;sq_settop(vm,0);fn.Null();
 std::cout<<"host collect "<<sq_collectgarbage(vm)<<' '<<alive(root_watch)<<' '<<alive(print_watch)<<'\n';
 sq_release(vm,&result);result.Null();
 std::cout<<"unrooted before "<<alive(root_watch)<<' '<<alive(print_watch)<<'\n';
 std::cout<<"unrooted collect "<<sq_collectgarbage(vm)<<' '<<alive(root_watch)<<' '<<alive(print_watch)<<'\n';
 return 0;
}

static int root_teardown_session() {
 SQObjectPtr root_watch,closure_watch;
 auto owner=std::unique_ptr<SQVM,decltype(&sq_close)>(sq_open(256),sq_close);auto vm=owner.get();
 {
  SQObjectPtr root=vm->_roottable,print;
  _table(root)->Get(SQObjectPtr(SQString::Create(_ss(vm),"print")),print);
  root_watch=_table(root)->GetWeakRef(OT_TABLE);closure_watch=_nativeclosure(print)->GetWeakRef(OT_NATIVECLOSURE);
  _table(root)->NewSlot(SQObjectPtr(SQString::Create(_ss(vm),"self")),root);
 }
 std::cout<<"root teardown before "<<(type(_weakref(root_watch)->_obj)==OT_TABLE)<<' '<<(type(_weakref(closure_watch)->_obj)==OT_NATIVECLOSURE)<<'\n';
 owner.reset();
 std::cout<<"root teardown after "<<(type(_weakref(root_watch)->_obj)==OT_TABLE)<<' '<<(type(_weakref(closure_watch)->_obj)==OT_NATIVECLOSURE)<<'\n';
 return 0;
}
