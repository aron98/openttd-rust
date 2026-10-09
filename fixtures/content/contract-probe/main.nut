/* SPDX-License-Identifier: GPL-2.0-only */
class ContractProbe extends AIController {
    restored = false;
    function Save() { return { contract_marker = 123 }; }
    function Load(version, data) {
        this.restored = version == 1 && data.rawin("contract_marker") && data.contract_marker == 123;
    }
    function Start() {
        print("CONTRACT_SPEED engine=0 speed=" + AIEngine.GetMaxSpeed(0));
        print("CONTRACT_RESTORED " + this.restored);
        if (!this.restored) {
            local changed = AICompany.SetName("Contract Probe");
            print("CONTRACT_RENAME result=" + changed + " name=" + AICompany.GetName(AICompany.COMPANY_SELF));
        } else {
            print("CONTRACT_COMPANY name=" + AICompany.GetName(AICompany.COMPANY_SELF));
        }
        while (true) this.Sleep(100);
    }
}
