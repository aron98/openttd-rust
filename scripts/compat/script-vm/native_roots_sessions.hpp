// Finite source-defined root changes and explicit friend retention modes.
namespace native_roots {
static bool persistent(HSQUIRRELVM vm) {
 if(!primary(vm,true))return false;
 const std::string_view source="try { throw [7]; } catch(e) {} return 1;";
 if(SQ_FAILED(sq_compilebuffer(vm,source,"roots-persistent",SQFalse)))return false;
 {
  SQObjectPtr main=stack_get(vm,-1);sq_settop(vm,0);sq_pushroottable(vm);
  SQObjectPtr result;vm->_can_suspend=true;vm->_ops_till_suspend=10000;
  const bool ok=vm->Call(main,1,vm->_top-1,result,SQFalse,SQTrue);
  std::cout<<"roots result "<<ok<<' ';value(result);
  if(!ok || vm->_suspended || type(vm->_lasterror)!=OT_ARRAY)return false;
  if(!owner_trace::add(_array(vm->_lasterror),'E'))return false;
  if(type(vm->temp_reg)==OT_CLOSURE && !owner_trace::add(_closure(vm->temp_reg),'P'))return false;
 }
 sq_settop(vm,0);collect(vm,"retained");
 const bool failed=SQ_FAILED(sq_compilebuffer(vm,"local = ;","roots-error",SQFalse));
 std::cout<<"roots compile failure "<<failed<<'\n';
 if(!failed)return false;
 collect(vm,"compile-error");
 if(SQ_FAILED(sq_compilebuffer(vm,"return 2;","roots-success",SQFalse)))return false;
 sq_pop(vm,1);collect(vm,"compile-success");
 sq_reseterror(vm);collect(vm,"reset");
 return true;
}
static bool payload(HSQUIRRELVM vm,SQTable *root,char label) {
 SQObjectPtr child=SQTable::Create(_ss(vm),0);
 if(!owner_trace::add(_table(child),label))return false;
 root->NewSlot(SQObjectPtr(SQInteger(0)),child);return true;
}
static bool replacement(HSQUIRRELVM vm,bool same) {
 if(!primary(vm,true) || !payload(vm,_table(vm->_roottable),'A'))return false;
 snapshot(vm,"before");
 if(same)sq_pushroottable(vm);
 else {
  sq_newtable(vm);
  if(!owner_trace::add(_table(stack_get(vm,-1)),'N') || !payload(vm,_table(stack_get(vm,-1)),'B'))return false;
 }
 if(SQ_FAILED(sq_setroottable(vm)))return false;
 owner_trace::emit();collect(vm,"replaced");return true;
}
static bool friend_context(HSQUIRRELVM vm,std::string_view mode) {
 if(!primary(vm,true))return false;
 auto child=sq_newthread(vm,256);
 if(child==nullptr || !owner_trace::add(child,'F'))return false;
 SQObjectPtr local;HSQOBJECT host; sq_resetobject(&host);
 if(mode=="friend-local") {local=stack_get(vm,-1);sq_pop(vm,1);}
 if(mode=="friend-host") {host=stack_get(vm,-1);sq_addref(vm,&host);sq_pop(vm,1);}
 snapshot(child,"friend-before");collect(vm,"primary-collected");
 // New idle friend: Finalize leaves scalar counters and allocated null slots safe.
 snapshot(child,"friend-after");
 if(mode=="friend-stack")sq_pop(vm,1);
 if(mode=="friend-local")local.Null();
 if(mode=="friend-host") {sq_release(vm,&host);sq_resetobject(&host);}
 owner_trace::emit();
 if(mode=="friend-local") {
  // The sole unmarked idle friend was disconnected; do not traverse or collect again.
  std::cout<<"roots detached-head "<<(_ss(vm)->_gc_chain==nullptr)<<' '<<owner_trace::label(_ss(vm)->_gc_chain)<<'\n';
  object("primary-root",vm->_roottable);
  if(_ss(vm)->_gc_chain!=nullptr){std::cout.flush();std::exit(65);}
 }else collect(vm,"friend-released");
 return true;
}
}
