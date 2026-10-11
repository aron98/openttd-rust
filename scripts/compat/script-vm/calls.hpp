// Passive original host-entry observations; no Squirrel behavior is replaced.
static int call_entry_owners(bool retained) {
 auto owner=std::unique_ptr<SQVM,decltype(&sq_close)>(sq_open(256),sq_close);auto vm=owner.get();
 SQObjectPtr key=SQString::Create(_ss(vm),"held"),held=SQTable::Create(_ss(vm),0);
 SQObjectPtr held_watch=_table(held)->GetWeakRef(OT_TABLE);_table(vm->_roottable)->NewSlot(key,held);held.Null();
 if(SQ_FAILED(sq_compilebuffer(vm,"type(held);\nreturn 0;","entry",SQFalse)))return 65;
 SQObjectPtr closure=stack_get(vm,-1);
 auto proto=_funcproto(_closure(closure)->_function);
 SQObjectPtr closure_watch=_closure(closure)->GetWeakRef(OT_CLOSURE),proto_watch=proto->GetWeakRef(OT_FUNCPROTO);
 if(!retained)sq_settop(vm,0);
 std::cout<<"before "<<_closure(closure)->_uiRef<<' '<<proto->_uiRef<<' '<<vm->_top<<'\n';
 sq_pushroottable(vm);SQObjectPtr result;vm->_can_suspend=true;vm->_ops_till_suspend=4;
 if(!vm->Call(closure,1,vm->_top-1,result,SQFalse,SQTrue)||!vm->_suspended)return 65;
 std::cout<<"suspended "<<vm->_ops_till_suspend<<' '<<(vm->ci->_ip-proto->_instructions)<<' '<<vm->_stackbase<<' '<<vm->_top<<' '<<_closure(closure)->_uiRef<<' '<<proto->_uiRef<<'\n';
 std::cout<<"temporary "<<type(vm->temp_reg)<<'\n';
 _table(vm->_roottable)->Remove(key);
 std::cout<<"argument after unbind "<<(type(_weakref(held_watch)->_obj)==OT_TABLE)<<'\n';
 vm->_ops_till_suspend+=100;
 if(!vm->Execute(_null_,vm->_top,-1,-1,result,SQFalse,SQVM::ET_RESUME_OPENTTD)||vm->_suspended)return 65;
 std::cout<<"returned "<<vm->_ops_till_suspend<<' '<<_closure(closure)->_uiRef<<' '<<proto->_uiRef<<' '<<(type(_weakref(held_watch)->_obj)==OT_TABLE)<<'\n';
 closure.Null();
 std::cout<<"local released "<<(type(_weakref(closure_watch)->_obj)==OT_CLOSURE)<<' '<<(type(_weakref(proto_watch)->_obj)==OT_FUNCPROTO)<<'\n';
 sq_settop(vm,0);
 std::cout<<"host cleared "<<(type(_weakref(closure_watch)->_obj)==OT_CLOSURE)<<' '<<(type(_weakref(proto_watch)->_obj)==OT_FUNCPROTO)<<'\n';
 return 0;
}
static int call_array_owners(bool retained) {
 auto owner=std::unique_ptr<SQVM,decltype(&sq_close)>(sq_open(256),sq_close);auto vm=owner.get();
 SQObjectPtr key=SQString::Create(_ss(vm),"held"),held=SQTable::Create(_ss(vm),0);
 SQObjectPtr held_watch=_table(held)->GetWeakRef(OT_TABLE);_table(vm->_roottable)->NewSlot(key,held);held.Null();
 if(SQ_FAILED(sq_compilebuffer(vm,"return array(2,held);","call-array",SQFalse)))return 65;
 SQObjectPtr closure=stack_get(vm,-1);auto proto=_funcproto(_closure(closure)->_function);
 if(!retained)sq_settop(vm,0);
 sq_pushroottable(vm);SQObjectPtr result;vm->_can_suspend=true;vm->_ops_till_suspend=5;
 if(!vm->Call(closure,1,vm->_top-1,result,SQFalse,SQTrue)||!vm->_suspended)return 65;
 std::cout<<"suspended "<<vm->_ops_till_suspend<<' '<<(vm->ci->_ip-proto->_instructions)<<'\n';
 SQObjectPtr array=vm->_stack[vm->_stackbase+proto->_instructions[3]._arg0];
 if(type(array)!=OT_ARRAY)return 65;
 SQObjectPtr array_watch=_array(array)->GetWeakRef(OT_ARRAY);
 std::cout<<"fill alias "<<(_table(_array(array)->_values[0])==_table(_array(array)->_values[1]))<<'\n';array.Null();
 _table(vm->_roottable)->Remove(key);
 std::cout<<"after unbind "<<(type(_weakref(held_watch)->_obj)==OT_TABLE)<<'\n';
 vm->_ops_till_suspend+=100;
 if(!vm->Execute(_null_,vm->_top,-1,-1,result,SQFalse,SQVM::ET_RESUME_OPENTTD)||vm->_suspended)return 65;
 std::cout<<"returned "<<vm->_ops_till_suspend<<' '<<(type(result)==OT_ARRAY)<<'\n';result.Null();
 std::cout<<"temporary held "<<(type(_weakref(array_watch)->_obj)==OT_ARRAY)<<' '<<(type(_weakref(held_watch)->_obj)==OT_TABLE)<<'\n';
 vm->temp_reg.Null();
 std::cout<<"temporary released "<<(type(_weakref(array_watch)->_obj)==OT_ARRAY)<<' '<<(type(_weakref(held_watch)->_obj)==OT_TABLE)<<'\n';
 return 0;
}
static int call_failure_session(int argc,char **argv) {
 if(argc!=3)return 64;
 std::ifstream input(argv[2],std::ios::binary);if(!input)return 66;
 std::string source((std::istreambuf_iterator<char>(input)),{});
 auto owner=std::unique_ptr<SQVM,decltype(&sq_close)>(sq_open(256),sq_close);auto vm=owner.get();
 SQObjectPtr closure,result;
 if(SQ_FAILED(sq_compilebuffer(vm,source,"call-failure",SQFalse)))return 65;
 closure=stack_get(vm,-1);sq_settop(vm,0);sq_pushroottable(vm);vm->_can_suspend=true;vm->_ops_till_suspend=10000;
 bool ok=vm->Call(closure,1,vm->_top-1,result,SQFalse,SQTrue);
 std::cout<<"failed "<<ok<<' '<<vm->_ops_till_suspend<<' '<<_stringval(vm->_lasterror)<<'\n';
 SQObjectPtr held,copy,key=SQString::Create(_ss(vm),"held"),copykey=SQString::Create(_ss(vm),"copy");
 if(!_table(vm->_roottable)->Get(key,held)||type(held)!=OT_TABLE)return 65;
 if(!_table(vm->_roottable)->Get(copykey,copy)||type(copy)!=OT_ARRAY)return 65;
 SQObjectPtr held_watch=_table(held)->GetWeakRef(OT_TABLE),copy_watch=_array(copy)->GetWeakRef(OT_ARRAY),found;
 if(!_table(held)->Get(SQObjectPtr(SQString::Create(_ss(vm),"key")),found)||type(found)!=OT_INTEGER)return 65;
 std::cout<<"retained "<<_integer(found)<<' '<<_array(copy)->Size()<<' '<<(_table(_array(copy)->_values[0])==_table(held))<<' '<<(_table(_array(copy)->_values[1])==_table(held))<<'\n';
 std::cout<<"temporary "<<type(vm->temp_reg)<<'\n';held.Null();copy.Null();
 _table(vm->_roottable)->Remove(key);_table(vm->_roottable)->Remove(copykey);
 std::cout<<"unbound "<<(type(_weakref(held_watch)->_obj)==OT_TABLE)<<' '<<(type(_weakref(copy_watch)->_obj)==OT_ARRAY)<<'\n';
 sq_settop(vm,0);
 if(SQ_FAILED(sq_compilebuffer(vm,"return type(7);","after-failure",SQFalse)))return 65;
 closure=stack_get(vm,-1);sq_settop(vm,0);sq_pushroottable(vm);vm->_ops_till_suspend=100;
 if(!vm->Call(closure,1,vm->_top-1,result,SQFalse,SQTrue)||type(result)!=OT_STRING)return 65;
 std::cout<<"next "<<vm->_ops_till_suspend<<' '<<_stringval(result)<<' '<<_stringval(vm->_lasterror)<<'\n';
 return 0;
}
static int call_cycle_teardown() {
 SQObjectPtr table_watch,array_watch;
 auto owner=std::unique_ptr<SQVM,decltype(&sq_close)>(sq_open(256),sq_close);auto vm=owner.get();
 {
 if(SQ_FAILED(sq_compilebuffer(vm,"local t={};\nlocal a=array(2,t);\nt.a<-a;\nreturn a;","call-cycle",SQFalse)))return 65;
 SQObjectPtr closure=stack_get(vm,-1);sq_settop(vm,0);sq_pushroottable(vm);SQObjectPtr result;
 vm->_can_suspend=true;vm->_ops_till_suspend=10000;
 if(!vm->Call(closure,1,vm->_top-1,result,SQFalse,SQTrue)||type(result)!=OT_ARRAY)return 65;
 SQObjectPtr leaf=_array(result)->_values[0];if(type(leaf)!=OT_TABLE)return 65;
 array_watch=_array(result)->GetWeakRef(OT_ARRAY);table_watch=_table(leaf)->GetWeakRef(OT_TABLE);
 std::cout<<"cycle returned "<<vm->_ops_till_suspend<<' '<<(_table(leaf)==_table(_array(result)->_values[1]))<<'\n';
 result.Null();leaf.Null();vm->temp_reg.Null();
 }
 std::cout<<"cycle before "<<(type(_weakref(table_watch)->_obj)==OT_TABLE)<<' '<<(type(_weakref(array_watch)->_obj)==OT_ARRAY)<<'\n';
 owner.reset();
 std::cout<<"cycle after "<<(type(_weakref(table_watch)->_obj)==OT_TABLE)<<' '<<(type(_weakref(array_watch)->_obj)==OT_ARRAY)<<'\n';
 return 0;
}
static int call_callee_owner() {
 auto owner=std::unique_ptr<SQVM,decltype(&sq_close)>(sq_open(256),sq_close);auto vm=owner.get();
 SQObjectPtr name=SQString::Create(_ss(vm),"type"),callee;
 if(!_table(vm->_roottable)->Get(name,callee)||type(callee)!=OT_NATIVECLOSURE)return 65;
 SQObjectPtr watch=_nativeclosure(callee)->GetWeakRef(OT_NATIVECLOSURE);callee.Null();
 if(SQ_FAILED(sq_compilebuffer(vm,"type(7);\nreturn 0;","call-callee",SQFalse)))return 65;
 SQObjectPtr closure=stack_get(vm,-1);sq_settop(vm,0);sq_pushroottable(vm);SQObjectPtr result;
 vm->_can_suspend=true;vm->_ops_till_suspend=4;
 if(!vm->Call(closure,1,vm->_top-1,result,SQFalse,SQTrue)||!vm->_suspended)return 65;
 std::cout<<"callee identity "<<(type(vm->temp_reg)==OT_NATIVECLOSURE&&_nativeclosure(vm->temp_reg)==_nativeclosure(_weakref(watch)->_obj))<<'\n';
 _table(vm->_roottable)->Remove(name);
 std::cout<<"callee unbound "<<(type(_weakref(watch)->_obj)==OT_NATIVECLOSURE)<<'\n';
 vm->_ops_till_suspend+=100;
 if(!vm->Execute(_null_,vm->_top,-1,-1,result,SQFalse,SQVM::ET_RESUME_OPENTTD)||vm->_suspended)return 65;
 std::cout<<"callee returned "<<vm->_ops_till_suspend<<' '<<(type(_weakref(watch)->_obj)==OT_NATIVECLOSURE)<<'\n';
 return 0;
}
