// Original table operations; borrowed checkpoints hold no additional graph owners.
static void delegate_snapshot(std::string_view phase) {
 std::cout<<"snapshot "<<phase<<'\n';
 for(const auto &entry:owner_trace::identities) {
  if(entry.label==0)continue;
  std::cout<<"object "<<entry.label<<' '<<(entry.pointer!=nullptr);
  if(entry.pointer!=nullptr) {
   auto table=static_cast<SQTable *>(entry.pointer);
   std::cout<<' '<<(table->_uiRef & ~MARK_FLAG)<<' '<<table->CountUsed()<<' '<<owner_trace::label(table->_delegate);
  }
  if(entry.label!=0)std::cout<<'\n';
 }
 owner_trace::emit();
}
static SQTable *delegate_table(HSQUIRRELVM vm,char label) {
 auto table=SQTable::Create(_ss(vm),4);
 if(!owner_trace::add(table,label))std::exit(65);
 return table;
}
static void delegate_chain(HSQUIRRELVM vm) {
 std::cout<<"chain";
 for(auto object=_ss(vm)->_gc_chain;object!=nullptr;object=object->_next) {
  auto label=owner_trace::label(object);
  if(label!='?')std::cout<<' '<<label;
 }
 std::cout<<'\n';
}
static int delegate_graph(std::string_view mode,bool record) {
 auto owner=std::unique_ptr<SQVM,decltype(&sq_close)>(sq_open(256),sq_close);auto vm=owner.get();
 owner_trace::begin(record);
 SQObjectPtr a=delegate_table(vm,'A'),b=delegate_table(vm,'B'),d;
 if(mode!="gc-own")d=delegate_table(vm,'D');
 SQObjectPtr o=delegate_table(vm,'O');
 auto table=_table(o);
 table->NewSlot(SQObjectPtr(SQInteger(0)),a);table->NewSlot(SQObjectPtr(SQInteger(1)),b);
 if(type(d)==OT_TABLE && !table->SetDelegate(_table(d)))return 65;
 if(mode!="ordinary")table->NewSlot(SQObjectPtr(SQInteger(2)),o);
 a.Null();b.Null();d.Null();delegate_snapshot("attached");
 if(mode=="marked") {
  _table(vm->_roottable)->NewSlot(SQObjectPtr(SQInteger(77)),o);o.Null();
  std::cout<<"collected reachable "<<sq_collectgarbage(vm)<<'\n';
  delegate_snapshot("rooted");delegate_chain(vm);
  _table(vm->_roottable)->Remove(SQObjectPtr(SQInteger(77)));
 } else o.Null();
 if(mode!="ordinary")std::cout<<"collected unreachable "<<sq_collectgarbage(vm)<<'\n';
 delegate_snapshot("released");
 return owner_trace::finish()?0:65;
}
static int delegate_mutations(bool record) {
 auto owner=std::unique_ptr<SQVM,decltype(&sq_close)>(sq_open(256),sq_close);auto vm=owner.get();
 owner_trace::begin(record);
 SQObjectPtr d=delegate_table(vm,'D'),n=delegate_table(vm,'N'),o=delegate_table(vm,'O');
 auto table=_table(o);
 if(!table->SetDelegate(_table(d)))return 65;
 d.Null();delegate_snapshot("attached");
 if(!table->SetDelegate(table->_delegate))return 65;
 delegate_snapshot("same");
 if(!table->SetDelegate(_table(n)))return 65;
 n.Null();delegate_snapshot("replaced");
 SQObjectPtr clone=table->Clone();if(!owner_trace::add(_table(clone),'C'))return 65;
 delegate_snapshot("cloned");
 const bool direct=table->SetDelegate(table);
 const bool transitive=table->_delegate->SetDelegate(table);
 std::cout<<"cycle refusal "<<direct<<' '<<transitive<<'\n';
 if(direct || transitive)return 65;
 delegate_snapshot("cycles");
 if(!table->SetDelegate(nullptr))return 65;
 delegate_snapshot("detached");
 o.Null();delegate_snapshot("owner released");
 clone.Null();delegate_snapshot("clone released");
 return owner_trace::finish()?0:65;
}
static int delegate_owner_session(int argc,char **argv) {
 if(argc!=4)return 64;
 const std::string_view mode=argv[2],record=argv[3];
 if(record!="on" && record!="off")return 64;
 if(mode=="ordinary" || mode=="gc" || mode=="marked" || mode=="gc-own")return delegate_graph(mode,record=="on");
 if(mode=="mutations")return delegate_mutations(record=="on");
 return 64;
}
