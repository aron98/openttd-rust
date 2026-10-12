// Borrowed live root snapshots; no graph owner is created by observation.
namespace native_roots {
static bool active_snapshot=false;
static void object(std::string_view name,const SQObject &value) {
 const bool reference=ISREFCOUNTED(type(value));
 const auto pointer=reference?_refcounted(value):nullptr;
 const auto refs=reference?(pointer->_uiRef & ~MARK_FLAG):0;
 SQInteger extent=-1;
 if(type(value)==OT_TABLE)extent=_table(value)->CountUsed();
 if(type(value)==OT_ARRAY)extent=_array(value)->Size();
 std::cout<<"roots object "<<name<<' '<<type(value)<<' '<<owner_trace::label(pointer)<<' '<<refs<<' '<<extent<<'\n';
 if(type(value)==OT_STRING) {
  static constexpr char hex[]="0123456789abcdef";
  const auto bytes=_stringval(value);
  std::cout<<"roots bytes "<<name<<' '<<bytes.size()<<' ';
  for(unsigned char byte:bytes)std::cout<<hex[byte>>4]<<hex[byte&15];
  std::cout<<'\n';
 }
}
static void chain(HSQUIRRELVM vm) {
 size_t count=0;
 for(auto node=_ss(vm)->_gc_chain;node!=nullptr;node=node->_next)++count;
 std::cout<<"roots chain "<<count<<' ';
 for(auto node=_ss(vm)->_gc_chain;node!=nullptr;node=node->_next)std::cout<<owner_trace::label(node);
 std::cout<<'\n';
}
static void snapshot(HSQUIRRELVM vm,std::string_view phase) {
 std::cout<<"roots begin "<<phase<<'\n';
 std::cout<<"roots vm "<<owner_trace::label(vm)<<' '<<(vm->_uiRef & ~MARK_FLAG)<<' '
  <<(_thread(_ss(vm)->_root_vm)==vm)<<' '<<vm->_stack.size()<<' '<<vm->_top<<' '<<vm->_stackbase<<' '
  <<vm->_vargsstack.size()<<' '<<vm->_callsstacksize<<' '<<vm->_etraps.size()<<' '<<vm->_ops_till_suspend<<'\n';
 object("error",vm->_lasterror);object("handler",vm->_errorhandler);object("debug",vm->_debughook);
 object("root",vm->_roottable);object("temporary",vm->temp_reg);
 for(SQUnsignedInteger i=0;i<vm->_stack.size();++i) {
  std::cout<<"roots stack "<<i<<'\n';object("slot",vm->_stack[i]);
 }
 for(SQUnsignedInteger i=0;i<vm->_vargsstack.size();++i) {
  std::cout<<"roots vararg "<<i<<'\n';object("slot",vm->_vargsstack[i]);
 }
 for(SQInteger i=0;i<vm->_callsstacksize;++i) {
  const auto &frame=vm->_callsstack[i];
  std::cout<<"roots frame "<<i<<' '<<frame._root<<' '<<frame._etraps<<' '<<frame._vargs.base<<' '<<frame._vargs.size<<'\n';
  object("closure",frame._closure);
 }
 chain(vm);std::cout<<"roots end "<<phase<<'\n';
}
static bool primary(HSQUIRRELVM vm,bool root) {
 return owner_trace::add(vm,'V') && (!root || owner_trace::add(_table(vm->_roottable),'O'));
}
static void collect(HSQUIRRELVM vm,std::string_view phase) {
 std::cout<<"roots collected "<<phase<<' '<<sq_collectgarbage(vm)<<'\n';owner_trace::emit();snapshot(vm,phase);
}
static bool shared(HSQUIRRELVM vm) {
 auto ss=_ss(vm);
 const std::array<std::pair<char,const SQObjectPtr *>,13> fields={{
  {'R',&ss->_registry},{'C',&ss->_consts},{'M',&ss->_metamethodsmap},
  {'T',&ss->_table_default_delegate},{'A',&ss->_array_default_delegate},{'S',&ss->_string_default_delegate},
  {'N',&ss->_number_default_delegate},{'F',&ss->_closure_default_delegate},{'G',&ss->_generator_default_delegate},
  {'H',&ss->_thread_default_delegate},{'K',&ss->_class_default_delegate},{'I',&ss->_instance_default_delegate},
  {'W',&ss->_weakref_default_delegate}}};
 if(!primary(vm,false))return false;
 for(const auto &[label,value]:fields)if(type(*value)!=OT_TABLE || !owner_trace::add(_table(*value),label))return false;
 snapshot(vm,"idle");
 for(const auto &[label,value]:fields)object(std::string_view(&label,1),*value);
 collect(vm,"shared-collected");
 return true;
}
}
