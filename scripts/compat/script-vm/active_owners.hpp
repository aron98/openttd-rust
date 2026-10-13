// Intentional fixed-marker host collection; the free recorder remains passive.
namespace active_owner {
static bool collect=false;
static unsigned callbacks=0;
static SQTable *local=nullptr;
static SQArray *local_array=nullptr;
static SQNativeClosure *local_native=nullptr;
static SQTable *host_table=nullptr;
static void state(HSQUIRRELVM vm,std::string_view phase) {
 if(native_roots::active_snapshot)native_roots::snapshot(vm,phase);
 std::cout<<"active "<<phase<<' '<<vm->_ops_till_suspend<<' '<<vm->_callsstacksize<<' '<<vm->_stackbase<<' '<<vm->_top<<' '<<vm->_vargsstack.size()<<'\n';
 for(SQUnsignedInteger i=0;i<vm->_stack.size();++i) {
  const auto &slot=vm->_stack[i];
  if(type(slot)==OT_TABLE) {
   const char label=owner_trace::label(_table(slot));
   if(label!='?')std::cout<<"stack-root "<<i<<' '<<label<<'\n';
  }
 }
 for(SQUnsignedInteger i=0;i<vm->_vargsstack.size();++i) {
  const auto &slot=vm->_vargsstack[i];
  if(type(slot)==OT_TABLE) {
   const char label=owner_trace::label(_table(slot));
   if(label!='?')std::cout<<"vararg-root "<<i<<' '<<label<<'\n';
  }
 }
 for(SQInteger frame=0;frame<vm->_callsstacksize;++frame) {
  const auto &value=vm->_callsstack[frame]._closure;
  if(type(value)!=OT_CLOSURE)continue;
  auto closure=_closure(value);
  for(SQUnsignedInteger i=0;i<closure->_outervalues.size();++i) {
   const auto &outer=closure->_outervalues[i];
   if(type(outer)==OT_TABLE)std::cout<<"capture-root "<<frame<<' '<<i<<' '<<owner_trace::label(_table(outer))<<'\n';
  }
  if(type(closure->_env)==OT_WEAKREF) {
   const auto &target=_weakref(closure->_env)->_obj;
   std::cout<<"weak-environment "<<frame<<' '<<type(target);
   if(type(target)==OT_TABLE)std::cout<<' '<<owner_trace::label(_table(target));
   std::cout<<'\n';
  }
 }
 if(type(vm->temp_reg)==OT_TABLE)std::cout<<"temporary-root "<<owner_trace::label(_table(vm->temp_reg))<<'\n';
 if(type(vm->_lasterror)==OT_TABLE)std::cout<<"error-root "<<owner_trace::label(_table(vm->_lasterror))<<'\n';
 if(host_table!=nullptr)std::cout<<"host-reference "<<(host_table->_uiRef & ~MARK_FLAG)<<' '<<host_table->CountUsed()<<' '<<owner_trace::label(host_table->_delegate)<<'\n';
 if(local_native!=nullptr)std::cout<<"native-reference "<<(local_native->_uiRef & ~MARK_FLAG)<<' '<<local_native->_nparamscheck<<' '<<local_native->_outervalues.size()<<' '<<_stringval(local_native->_name)<<'\n';
 if(local_array!=nullptr)std::cout<<"array-reference "<<(local_array->_uiRef & ~MARK_FLAG)<<' '<<local_array->Size()<<'\n';
 if(local!=nullptr)std::cout<<"local-reference "<<(local->_uiRef & ~MARK_FLAG)<<' '<<local->CountUsed()<<' '<<owner_trace::label(local->_delegate)<<'\n';
 delegate_chain(vm);owner_trace::emit();
}
static void print(HSQUIRRELVM vm,std::string_view bytes) {
 if(bytes!="collect-active" || callbacks!=0)std::exit(65);
 ++callbacks;
 std::cout<<"print "<<bytes<<'\n';state(vm,"before");
 if(collect)std::cout<<"collected active "<<sq_collectgarbage(vm)<<'\n';
 else std::cout<<"collection disabled\n";
 state(vm,"after");
}
static int session(int argc,char **argv) {
 if(argc!=6 && argc!=7)return 64;
 const std::string_view mode=argv[2],action=argv[4],record=argv[5];
 if((mode!="local" && mode!="script" && mode!="host" && mode!="handler" && mode!="local-array" && mode!="local-native" && mode!="local-array-acyclic" && mode!="local-host") || (action!="on" && action!="off") || (record!="on" && record!="off"))return 64;
 std::ifstream input(argv[3],std::ios::binary);if(!input)return 66;
 const std::string source((std::istreambuf_iterator<char>(input)),{});
 auto owner=std::unique_ptr<SQVM,decltype(&sq_close)>(sq_open(256),sq_close);auto vm=owner.get();
 if(mode=="handler") {
  if(argc!=7)return 64;
  std::ifstream handler_input(argv[6],std::ios::binary);if(!handler_input)return 66;
  const std::string handler_source((std::istreambuf_iterator<char>(handler_input)),{});
  if(SQ_FAILED(sq_compilebuffer(vm,handler_source,"active-handler",SQFalse)))return 65;
  sq_pushroottable(vm);
  if(SQ_FAILED(sq_call(vm,1,SQTrue,SQFalse,10000)))return 65;
  sq_seterrorhandler(vm);sq_settop(vm,0);vm->temp_reg.Null();
 }
 if(SQ_FAILED(sq_compilebuffer(vm,source,"active-owner",SQFalse)))return 65;
 SQObjectPtr main=stack_get(vm,-1);sq_settop(vm,0);
 owner_trace::begin(record=="on");
 SQObjectPtr d=delegate_table(vm,'D'),o=delegate_table(vm,'O');
 if(!_table(o)->SetDelegate(_table(d)))return 65;
 _table(o)->NewSlot(SQObjectPtr(SQInteger(0)),o);
 SQObjectPtr bundle=SQArray::Create(_ss(vm),2);
 _array(bundle)->Set(0,o);_array(bundle)->Set(1,d);
 const SQObjectPtr key=SQString::Create(_ss(vm),"__owner_setup");
 _table(vm->_roottable)->NewSlot(key,bundle);
 SQObjectPtr retained;
 if(mode=="local" || mode=="local-host") {retained=o;local=_table(retained);}
 if(mode=="local-array" || mode=="local-array-acyclic") {
  if(mode=="local-array-acyclic") {
   _table(o)->Remove(SQObjectPtr(SQInteger(0)));
   if(!_table(o)->SetDelegate(nullptr))return 65;
  }
  retained=bundle;local_array=_array(retained);if(!owner_trace::add(local_array,'A'))return 65;
 }
 if(mode=="local-native") {
  const SQObjectPtr name=SQString::Create(_ss(vm),"array");
  if(!_table(vm->_roottable)->Get(name,retained) || type(retained)!=OT_NATIVECLOSURE)return 65;
  local_native=_nativeclosure(retained);if(!owner_trace::add(local_native,'N'))return 65;
  _table(vm->_roottable)->Remove(name);
 }
 HSQOBJECT host=o;
 if(mode=="host" || mode=="local-host") {sq_addref(vm,&host);host_table=_table(host);}
 if(mode=="local" || mode=="host" || mode=="local-array" || mode=="local-native" || mode=="local-array-acyclic" || mode=="local-host")_table(vm->_roottable)->Remove(key);
 bundle.Null();o.Null();d.Null();
 collect=action=="on";callbacks=0;sq_setprintfunc(vm,print);
 sq_pushroottable(vm);SQObjectPtr result;vm->_can_suspend=true;vm->_ops_till_suspend=10000;
 const bool ok=vm->Call(main,1,vm->_top-1,result,mode=="handler",SQTrue);
 std::cout<<"completed "<<ok<<' '<<vm->_suspended<<' '<<vm->_ops_till_suspend<<' ';value(result);
 if(ok==(mode=="handler") || vm->_suspended || callbacks!=1)return 65;
 if(mode=="local") {
  SQObjectPtr found;
  const bool present=_table(retained)->Get(SQObjectPtr(SQInteger(0)),found);
  std::cout<<"own-get "<<present<<' ';value(found);found.Null();
  SQObjectPtr clone=_table(retained)->Clone();
  if(!owner_trace::add(_table(clone),'C'))return 65;
  std::cout<<"clone-used "<<_table(clone)->CountUsed()<<' '<<owner_trace::label(_table(clone)->_delegate)<<'\n';
  clone.Null();owner_trace::emit();
  _table(retained)->NewSlot(SQObjectPtr(SQInteger(1)),SQObjectPtr(SQInteger(9)));
  if(!_table(retained)->Get(SQObjectPtr(SQInteger(1)),found))return 65;
  std::cout<<"insert-used "<<_table(retained)->CountUsed()<<' ';value(found);
 }
 main.Null();result.Null();vm->temp_reg.Null();sq_settop(vm,0);sq_reseterror(vm);
 _table(vm->_roottable)->Remove(key);
 retained.Null();local=nullptr;local_array=nullptr;local_native=nullptr;
 if(mode=="host" || mode=="local-host") {sq_release(vm,&host);host_table=nullptr;}
 std::cout<<"released local owners\n";owner_trace::emit();
 std::cout<<"collected released "<<sq_collectgarbage(vm)<<'\n';
 owner_trace::emit();
 return owner_trace::finish()?0:65;
}
}
