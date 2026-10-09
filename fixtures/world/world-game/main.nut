/* SPDX-License-Identifier: GPL-2.0-only */
class WorldGame extends GSController {
    state = null;
    function Check(ok, operation) {
        if (!ok) throw operation + ": " + GSError.GetLastErrorString();
        print("WORLD_GAME_CHECK " + operation + " true");
    }
    function Save() { return this.state; }
    function Load(version, data) {
        if (version != 1 || data == null || data.marker != 362 ||
            data.nested.signed != -7 || data.nested.truth != true ||
            data.nested.nothing != null || data.nested.words[1] != "state" ||
            data.wide != 9007199254740993) throw "Invalid WorldGame saved data";
        this.state = data;
    }
    function Start() {
        if (this.state != null) {
            this.Check(GSGoal.IsValidGoal(this.state.goal), "restored-goal");
            this.Check(GSStoryPage.IsValidStoryPage(this.state.page), "restored-page");
            this.Check(GSStoryPage.IsValidStoryPageElement(this.state.element), "restored-element");
            this.Check(GSLeagueTable.IsValidLeagueTable(this.state.league), "restored-league");
            this.Check(GSLeagueTable.IsValidLeagueTableElement(this.state.entry), "restored-entry");
            print("WORLD_GAME_RESTORED marker=362 wide=" + this.state.wide);
            while (true) this.Sleep(100);
        }
        local page = GSStoryPage.New(GSCompany.COMPANY_INVALID, "World story");
        this.Check(GSStoryPage.IsValidStoryPage(page), "create-page");
        local goal = GSGoal.New(GSCompany.COMPANY_INVALID, "World goal", GSGoal.GT_STORY_PAGE, page);
        this.Check(GSGoal.IsValidGoal(goal), "create-goal");
        local element = GSStoryPage.NewElement(page, GSStoryPage.SPET_GOAL, goal, null);
        this.Check(GSStoryPage.IsValidStoryPageElement(element), "create-element");
        local league = GSLeagueTable.New("World league", "Saved header", "Saved footer");
        this.Check(GSLeagueTable.IsValidLeagueTable(league), "create-league");
        local entry = GSLeagueTable.NewElement(league, 362, GSCompany.COMPANY_INVALID,
            "World entry", "362 points", GSLeagueTable.LINK_STORY_PAGE, page);
        this.Check(GSLeagueTable.IsValidLeagueTableElement(entry), "create-entry");
        this.state = { marker = 362, wide = 9007199254740993, goal = goal, page = page,
            element = element, league = league, entry = entry,
            nested = { signed = -7, truth = true, nothing = null, words = ["world", "state"] } };
        print("WORLD_GAME_READY marker=362 goal=" + goal + " page=" + page + " league=" + league);
        while (true) this.Sleep(100);
    }
}
