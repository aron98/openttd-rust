// Passive test-only observations of the actual bundled closure VM.
static std::string function_path(SQFunctionProto *root,SQFunctionProto *target,std::string prefix="main") {
 if(root==target)return prefix;
 for(SQInteger i=0;i<root->_nfunctions;i++) {
  auto path=function_path(_funcproto(root->_functions[i]),target,prefix+"."+std::to_string(i));
  if(!path.empty())return path;
 }
 return {};
}
static void function_metadata(SQFunctionProto *proto,std::string prefix="main") {
 std::cout<<"prototype "<<prefix<<' '<<proto->_stacksize<<' '<<proto->_nparameters<<' '<<proto->_nfunctions<<' '<<proto->_noutervalues<<' '<<proto->_ndefaultparams<<' '<<proto->_varparams<<'\n';
 std::cout<<"source "<<prefix<<' ';value(proto->_sourcename);
 std::cout<<"name "<<prefix<<' ';value(proto->_name);
 for(SQInteger i=0;i<proto->_nparameters;i++){std::cout<<"parameter "<<prefix<<' '<<i<<' ';value(proto->_parameters[i]);}
 for(SQInteger i=0;i<proto->_nlocalvarinfos;i++){std::cout<<"local "<<prefix<<' '<<i<<' ';value(proto->_localvarinfos[i]._name);}
 for(SQInteger i=0;i<proto->_noutervalues;i++){std::cout<<"capture-name "<<prefix<<' '<<i<<' ';value(proto->_outervalues[i]._name);}
 for(SQInteger i=0;i<proto->_noutervalues;i++) {std::cout<<"outer "<<prefix<<' '<<i<<' '<<proto->_outervalues[i]._type<<' ';value(proto->_outervalues[i]._src);}
 for(SQInteger i=0;i<proto->_ndefaultparams;i++)std::cout<<"default "<<prefix<<' '<<i<<' '<<proto->_defaultparams[i]<<'\n';
 for(SQInteger n=0;n<proto->_ninstructions;n++){const auto &i=proto->_instructions[n];std::cout<<"child-op "<<prefix<<' '<<unsigned(i.op)<<' '<<unsigned(i._arg0)<<' '<<i._arg1<<' '<<unsigned(i._arg2)<<' '<<unsigned(i._arg3)<<'\n';}
 for(SQInteger i=0;i<proto->_nfunctions;i++)function_metadata(_funcproto(proto->_functions[i]),prefix+"."+std::to_string(i));
}

static int lifetime(bool cycle) {
 auto owner=std::unique_ptr<SQVM,decltype(&sq_close)>(sq_open(256),sq_close);auto vm=owner.get();
 const char *source=cycle?"local t={};\nlocal f=function():(t){return t;};\nt.f<-f;\nreturn f;":"local t={key=7};\nreturn function():(t){return t;};";
 if(SQ_FAILED(sq_compilebuffer(vm,source,"closure-owner",SQFalse)))return 65;
 SQObjectPtr main=stack_get(vm,-1);sq_settop(vm,0);sq_pushroottable(vm);SQObjectPtr result;
 vm->_can_suspend=true;vm->_ops_till_suspend=10000;
 if(!vm->Call(main,1,vm->_top-1,result,SQFalse,SQTrue)||type(result)!=OT_CLOSURE)return 65;
 auto closure=_closure(result);auto proto=_funcproto(closure->_function);auto leaf=_table(closure->_outervalues[0]);
 SQObjectPtr closure_watch=closure->GetWeakRef(OT_CLOSURE),proto_watch=proto->GetWeakRef(OT_FUNCPROTO),leaf_watch=leaf->GetWeakRef(OT_TABLE);
 std::cout<<"owners "<<_closure(main)->_uiRef<<' '<<closure->_uiRef<<' '<<proto->_uiRef<<' '<<leaf->_uiRef<<'\n';
 main.Null();std::cout<<"main released proto "<<proto->_uiRef<<'\n';
 result.Null();std::cout<<"result released closure "<<closure->_uiRef<<'\n';
 vm->temp_reg.Null();
 std::cout<<"temporary released "<<(type(_weakref(closure_watch)->_obj)==OT_CLOSURE)<<' '<<(type(_weakref(proto_watch)->_obj)==OT_FUNCPROTO)<<' '<<(type(_weakref(leaf_watch)->_obj)==OT_TABLE)<<'\n';
 if(cycle)std::cout<<"collected "<<sq_collectgarbage(vm)<<'\n';
 std::cout<<"final "<<(type(_weakref(closure_watch)->_obj)==OT_CLOSURE)<<' '<<(type(_weakref(proto_watch)->_obj)==OT_FUNCPROTO)<<' '<<(type(_weakref(leaf_watch)->_obj)==OT_TABLE)<<'\n';
 return 0;
}
static int failed_capture() {
 auto owner=std::unique_ptr<SQVM,decltype(&sq_close)>(sq_open(256),sq_close);auto vm=owner.get();
 SQObjectPtr key=SQString::Create(_ss(vm),"held"),table=SQTable::Create(_ss(vm),0),watch=_table(table)->GetWeakRef(OT_TABLE);
 _table(vm->_roottable)->NewSlot(key,table);table.Null();
 if(SQ_FAILED(sq_compilebuffer(vm,"return function():(held,missing){return held;};","failed-capture",SQFalse)))return 65;
 SQObjectPtr closure=stack_get(vm,-1);sq_settop(vm,0);sq_pushroottable(vm);SQObjectPtr result;vm->_can_suspend=true;vm->_ops_till_suspend=10000;
 bool ok=vm->Call(closure,1,vm->_top-1,result,SQFalse,SQTrue);
 std::cout<<"result "<<ok<<' '<<vm->_ops_till_suspend<<' '<<_stringval(vm->_lasterror)<<'\n';
 _table(vm->_roottable)->Remove(key);closure.Null();vm->temp_reg.Null();
 std::cout<<"held after error "<<(type(_weakref(watch)->_obj)==OT_TABLE)<<'\n';
 std::cout<<"collected "<<sq_collectgarbage(vm)<<'\n';
 std::cout<<"held after collection "<<(type(_weakref(watch)->_obj)==OT_TABLE)<<'\n';
 return 0;
}
static int prototype_view() {
 auto owner=std::unique_ptr<SQVM,decltype(&sq_close)>(sq_open(256),sq_close);auto vm=owner.get();
 if(SQ_FAILED(sq_compilebuffer(vm,"return function(){return 7;};","prototype-view",SQFalse)))return 65;
 SQObjectPtr main=stack_get(vm,-1);sq_settop(vm,0);sq_pushroottable(vm);SQObjectPtr result;
 vm->_can_suspend=true;vm->_ops_till_suspend=10000;
 if(!vm->Call(main,1,vm->_top-1,result,SQFalse,SQTrue)||type(result)!=OT_CLOSURE)return 65;
 auto closure=_closure(result);auto proto=_funcproto(closure->_function);
 SQObjectPtr closure_watch=closure->GetWeakRef(OT_CLOSURE),proto_watch=proto->GetWeakRef(OT_FUNCPROTO);
 std::cout<<"before view "<<closure->_uiRef<<' '<<proto->_uiRef<<'\n';
 SQObjectPtr view=closure->_function;
 std::cout<<"with view "<<closure->_uiRef<<' '<<proto->_uiRef<<'\n';
 main.Null();result.Null();vm->temp_reg.Null();
 std::cout<<"owners released "<<(type(_weakref(closure_watch)->_obj)==OT_CLOSURE)<<' '<<proto->_uiRef<<' '<<proto->_ninstructions<<'\n';
 view.Null();
 std::cout<<"view released "<<(type(_weakref(proto_watch)->_obj)==OT_FUNCPROTO)<<'\n';
 return 0;
}
static int default_cycle() {
 auto owner=std::unique_ptr<SQVM,decltype(&sq_close)>(sq_open(256),sq_close);auto vm=owner.get();
 const bool cycle=true;
 const char *source="local t={};\nlocal f=function(a=t){return a;};\nt.f<-f;\nreturn f;";
 if(SQ_FAILED(sq_compilebuffer(vm,source,"closure-owner",SQFalse)))return 65;
 SQObjectPtr main=stack_get(vm,-1);sq_settop(vm,0);sq_pushroottable(vm);SQObjectPtr result;
 vm->_can_suspend=true;vm->_ops_till_suspend=10000;
 if(!vm->Call(main,1,vm->_top-1,result,SQFalse,SQTrue)||type(result)!=OT_CLOSURE)return 65;
 auto closure=_closure(result);auto proto=_funcproto(closure->_function);auto leaf=_table(closure->_defaultparams[0]);
 SQObjectPtr closure_watch=closure->GetWeakRef(OT_CLOSURE),proto_watch=proto->GetWeakRef(OT_FUNCPROTO),leaf_watch=leaf->GetWeakRef(OT_TABLE);
 std::cout<<"owners "<<_closure(main)->_uiRef<<' '<<closure->_uiRef<<' '<<proto->_uiRef<<' '<<leaf->_uiRef<<'\n';
 main.Null();std::cout<<"main released proto "<<proto->_uiRef<<'\n';
 result.Null();std::cout<<"result released closure "<<closure->_uiRef<<'\n';
 vm->temp_reg.Null();
 std::cout<<"temporary released "<<(type(_weakref(closure_watch)->_obj)==OT_CLOSURE)<<' '<<(type(_weakref(proto_watch)->_obj)==OT_FUNCPROTO)<<' '<<(type(_weakref(leaf_watch)->_obj)==OT_TABLE)<<'\n';
 if(cycle)std::cout<<"collected "<<sq_collectgarbage(vm)<<'\n';
 std::cout<<"final "<<(type(_weakref(closure_watch)->_obj)==OT_CLOSURE)<<' '<<(type(_weakref(proto_watch)->_obj)==OT_FUNCPROTO)<<' '<<(type(_weakref(leaf_watch)->_obj)==OT_TABLE)<<'\n';
 return 0;
}
static int persistent_closure_session(bool failure) {
 auto owner=std::unique_ptr<SQVM,decltype(&sq_close)>(sq_open(256),sq_close);auto vm=owner.get();
 auto root=_table(vm->_roottable);
 const char *first="held<-{x=1};\nsaved<-function():(held){return held;};\nreturn 0;";
 if(SQ_FAILED(sq_compilebuffer(vm,first,"first",SQFalse)))return 65;
 SQObjectPtr main=stack_get(vm,-1);sq_settop(vm,0);sq_pushroottable(vm);SQObjectPtr result;
 vm->_can_suspend=true;vm->_ops_till_suspend=10000;
 if(!vm->Call(main,1,vm->_top-1,result,SQFalse,SQTrue))return 65;
 SQObjectPtr saved,key=SQString::Create(_ss(vm),"saved"),heldkey=SQString::Create(_ss(vm),"held"),held;
 if(!root->Get(key,saved)||!root->Get(heldkey,held))return 65;
 auto closure=_closure(saved);auto proto=_funcproto(closure->_function);auto leaf=_table(held);
 SQObjectPtr closure_watch=closure->GetWeakRef(OT_CLOSURE),proto_watch=proto->GetWeakRef(OT_FUNCPROTO),leaf_watch=leaf->GetWeakRef(OT_TABLE);
 root->Remove(heldkey);held.Null();main.Null();
 std::cout<<"persisted "<<closure->_uiRef<<' '<<proto->_uiRef<<' '<<leaf->_uiRef<<'\n';
 const char *second=failure?"local f=::saved;\ndelete ::saved;\nlocal t=f();\nt.x=9;\nassert(false);\nreturn t;":"local f=::saved;\ndelete ::saved;\nlocal t=f();\nt.x=9;\nreturn t;";
 sq_settop(vm,0);
 if(SQ_FAILED(sq_compilebuffer(vm,second,"second",SQFalse)))return 65;
 main=stack_get(vm,-1);sq_settop(vm,0);sq_pushroottable(vm);
 const SQInteger credits[]={0,1,2,4,0,2,2,2,2,2,2,2,2,16,32,10000};
 bool started=false;
 for(auto credit:credits) {
  vm->_ops_till_suspend=started?vm->_ops_till_suspend+credit:credit;
  bool ok=started?vm->Execute(_null_,vm->_top,-1,-1,result,SQFalse,SQVM::ET_RESUME_OPENTTD):vm->Call(main,1,vm->_top-1,result,SQFalse,SQTrue);
  started=true;
  if(!ok){std::cout<<"failed "<<vm->_ops_till_suspend<<' '<<closure->_uiRef<<' '<<proto->_uiRef<<' '<<leaf->_uiRef<<' '<<_stringval(vm->_lasterror)<<'\n';break;}
  if(!vm->_suspended){std::cout<<"returned "<<vm->_ops_till_suspend<<' '<<closure->_uiRef<<' '<<proto->_uiRef<<' '<<leaf->_uiRef<<'\n';break;}
  auto active=_closure(vm->ci->_closure);auto active_proto=_funcproto(active->_function);
  std::cout<<"suspended "<<vm->_ops_till_suspend<<' '<<(active==closure)<<' '<<(vm->ci->_ip-active_proto->_instructions)<<' '<<vm->_callsstacksize<<' '<<vm->_stackbase<<' '<<vm->_top<<' '<<closure->_uiRef<<' '<<proto->_uiRef<<' '<<leaf->_uiRef<<'\n';
 }
 SQObjectPtr x;
 if(!leaf->Get(SQString::Create(_ss(vm),"x"),x))return 65;
 std::cout<<"mutation ";value(x);
 main.Null();saved.Null();
 std::cout<<"callee released "<<(type(_weakref(closure_watch)->_obj)==OT_CLOSURE)<<' '<<(type(_weakref(proto_watch)->_obj)==OT_FUNCPROTO)<<' '<<(type(_weakref(leaf_watch)->_obj)==OT_TABLE?_table(_weakref(leaf_watch)->_obj)->_uiRef:0)<<'\n';
 result.Null();vm->temp_reg.Null();sq_settop(vm,0);
 std::cout<<"final "<<(type(_weakref(leaf_watch)->_obj)==OT_TABLE)<<'\n';
 return 0;
}
static int closure_compile_publication() {
 auto owner=std::unique_ptr<SQVM,decltype(&sq_close)>(sq_open(256),sq_close);auto vm=owner.get();
 const char *source="local x=1;\nlocal f=function():(x){x=function(){const Published=7;\nreturn 2;};};";
 const bool failed=SQ_FAILED(sq_compilebuffer(vm,source,"publication",SQFalse));
 std::cout<<"compile failed "<<failed<<'\n';
 SQObjectPtr found;
 bool present=_table(_ss(vm)->_consts)->Get(SQString::Create(_ss(vm),"Published"),found);
 std::cout<<"published "<<present<<' ';value(found);
 return failed&&present?0:65;
}
static int compound_store_errors() {
 auto owner=std::unique_ptr<SQVM,decltype(&sq_close)>(sq_open(256),sq_close);auto vm=owner.get();
 const char *sources[]={"const C=3;\nlocal f=function(){this=7;\nreturn this.C+=1;};\nreturn f();","return this.C+=1;"};
 for(auto source:sources) {
  sq_settop(vm,0);
  if(SQ_FAILED(sq_compilebuffer(vm,source,"compound-store",SQFalse)))return 65;
  SQObjectPtr main=stack_get(vm,-1);sq_settop(vm,0);sq_pushroottable(vm);SQObjectPtr result;
  vm->_can_suspend=true;vm->_ops_till_suspend=10000;
  bool ok=vm->Call(main,1,vm->_top-1,result,SQFalse,SQTrue);
  std::cout<<"returned "<<ok<<' '<<vm->_ops_till_suspend<<' ';value(result);
  std::cout<<"last error ";value(vm->_lasterror);
  if(!ok)return 65;
 }
 return 0;
}

static int trap_error_owner(int argc,char **argv) {
 if(argc!=3)return 64;
 std::ifstream input(argv[2],std::ios::binary);if(!input)return 66;
 std::string source((std::istreambuf_iterator<char>(input)),{});
 auto owner=std::unique_ptr<SQVM,decltype(&sq_close)>(sq_open(256),sq_close);auto vm=owner.get();
 if(SQ_FAILED(sq_compilebuffer(vm,source,"trap-owner",SQFalse)))return 65;
 SQObjectPtr main=stack_get(vm,-1);sq_settop(vm,0);sq_pushroottable(vm);SQObjectPtr result;
 vm->_can_suspend=true;vm->_ops_till_suspend=10000;
 bool ok=vm->Call(main,1,vm->_top-1,result,SQFalse,SQTrue);
 std::cout<<"completed "<<ok<<' '<<vm->_callsstacksize<<' '<<vm->_etraps.size()<<' ';value(vm->_lasterror);
 if(type(vm->_lasterror)!=OT_ARRAY)return 65;
 SQObjectPtr watch=_array(vm->_lasterror)->GetWeakRef(OT_ARRAY);
 main.Null();result.Null();vm->temp_reg.Null();sq_settop(vm,0);sq_collectgarbage(vm);
 std::cout<<"error retains "<<(type(_weakref(watch)->_obj)==OT_ARRAY)<<' '<<_array(vm->_lasterror)->_uiRef<<'\n';
 sq_reseterror(vm);sq_collectgarbage(vm);
 std::cout<<"error reset "<<(type(_weakref(watch)->_obj)==OT_ARRAY)<<'\n';
 return 0;
}
