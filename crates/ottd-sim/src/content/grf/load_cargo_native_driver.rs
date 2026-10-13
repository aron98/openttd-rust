use super::super::super::{load::ActionResult, records::Reader};
use super::{
    Budget, CargoState, Change, FileIdentity, LoadLocation, LoadStage, Registry, Request, Result,
    RoadRequest, Specs, Value, compare, json, location, number, projection, result_detail,
};
use std::collections::VecDeque;

pub(super) struct Harness<'a, 'n> {
    pub cargo: CargoState,
    pub registry: Registry<'a>,
    pub specs: Specs,
    pub budget: Budget,
    pub events: VecDeque<&'n Value>,
    pub compared: usize,
    pub name: &'n str,
    pub rows: Vec<Value>,
}

impl Harness<'_, '_> {
    fn check_specs(&mut self, phase: &str, detail: Option<&Value>, specs: &Specs) -> Result {
        let event = self.events.pop_front().ok_or("missing native event")?;
        assert_eq!(event.get("phase").and_then(Value::as_str), Some(phase));
        if let Some(detail) = detail {
            assert_eq!(event.get("detail"), Some(detail), "{}/{phase}", self.name);
        }
        let state = projection(&self.cargo, &self.registry, specs)?;
        compare(&state, event, self.name, phase)?;
        self.rows
            .push(json!({"phase":phase,"detail":detail,"state":state}));
        self.compared = self.compared.checked_add(1).ok_or("comparison count")?;
        Ok(())
    }

    fn check(&mut self, phase: &str, detail: Option<&Value>) -> Result {
        let event = self.events.pop_front().ok_or("missing native event")?;
        assert_eq!(event.get("phase").and_then(Value::as_str), Some(phase));
        if let Some(detail) = detail {
            assert_eq!(event.get("detail"), Some(detail), "{}/{phase}", self.name);
        }
        let state = projection(&self.cargo, &self.registry, &self.specs)?;
        compare(&state, event, self.name, phase)?;
        self.rows
            .push(json!({"phase":phase,"detail":detail,"state":state}));
        self.compared = self.compared.checked_add(1).ok_or("comparison count")?;
        Ok(())
    }

    pub(super) fn command(&mut self, command: &Value) -> Result {
        let op = command
            .get("op")
            .and_then(Value::as_str)
            .ok_or("operation")?;
        let mut detail = json!({"command":command});
        match op {
            "reset-default" | "reset-retained" | "snapshot" => (),
            "override" => {
                self.specs
                    .grfid_overrides
                    .insert(number(command, "source")?, number(command, "target")?);
            }
            "translate" | "build-inverse" => self.translate(command, op, &mut detail)?,
            "property" => {
                let outcome = self.property(command)?;
                detail
                    .as_object_mut()
                    .ok_or("detail")?
                    .extend(outcome.as_object().ok_or("outcome")?.clone());
            }
            _ => return Err("operation".into()),
        }
        self.check("api-command", Some(&detail))
    }

    fn translate(&mut self, command: &Value, op: &str, detail: &mut Value) -> Result {
        let grfid = number(command, "grfid")?;
        let file = self
            .registry
            .files
            .iter_mut()
            .find(|file| file.grfid == grfid)
            .ok_or("file")?;
        let table = file.cargo.as_mut().ok_or("table")?;
        if op == "translate" {
            let cargo = table.translate(
                u8::try_from(number(command, "cargo")?)?,
                command
                    .get("usebit")
                    .and_then(Value::as_bool)
                    .ok_or("usebit")?,
                file.version,
                &self.cargo,
            );
            detail
                .as_object_mut()
                .ok_or("detail")?
                .insert("cargo".into(), json!(cargo));
        } else {
            table.rebuild_inverse(file.version, &self.cargo);
        }
        Ok(())
    }

    fn property(&mut self, command: &Value) -> Result<Value> {
        let grfid = number(command, "grfid")?;
        let index = self
            .registry
            .files
            .iter()
            .position(|file| file.grfid == grfid)
            .ok_or("file")?;
        let stage = if command.get("stage").and_then(Value::as_str) == Some("reserve") {
            LoadStage::Reserve
        } else {
            LoadStage::Activation
        };
        let loc = location(stage, index);
        let request = Request {
            first: u16::try_from(number(command, "first")?)?,
            count: u16::try_from(number(command, "count")?)?,
            property: u8::try_from(number(command, "property")?)?,
        };
        let raw: Vec<u8> = serde_json::from_value(command.get("raw").ok_or("raw")?.clone())?;
        let mut reader = Reader {
            bytes: &raw,
            pos: 0,
        };
        let feature = number(command, "feature")?;
        let mut scope = json!({"feature":feature,"first":request.first,"last":u32::from(request.first).saturating_add(u32::from(request.count)),"property":request.property,"remaining":reader.remaining(),"result":-1,"unwinding":false});
        self.check("property-enter", Some(&scope))?;
        let outcome = self.invoke(feature, grfid, request, &mut reader, loc)?;
        let fields = scope.as_object_mut().ok_or("scope")?;
        fields.insert("remaining".into(), json!(reader.remaining()));
        fields.insert(
            "result".into(),
            outcome.get("result").cloned().unwrap_or_else(|| json!(-1)),
        );
        fields.insert(
            "unwinding".into(),
            json!(
                outcome
                    .get("read_bounds")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            ),
        );
        self.check("property-return", Some(&scope))?;
        Ok(outcome)
    }

    fn invoke(
        &mut self,
        feature: u32,
        grfid: u32,
        request: Request,
        reader: &mut Reader<'_>,
        loc: LoadLocation,
    ) -> Result<Value> {
        let result = match feature {
            11 => {
                let file = self.registry.file_mut(loc.file).ok_or("file")?;
                self.cargo.property(
                    file.cargo.as_mut().ok_or("table")?,
                    super::super::super::load_cargo::Binding {
                        identity: FileIdentity {
                            index: loc.file,
                            grfid,
                        },
                        version: file.version,
                    },
                    request,
                    reader,
                    &mut self.budget,
                    loc,
                )
            }
            8 => super::super::super::load_cargo_translation::apply_translation(
                &mut self.registry,
                &self.specs.grfid_overrides,
                request,
                reader,
                &mut self.budget,
                loc,
            ),
            1 => return self.road(grfid, request, reader, loc),
            _ => return Err("feature".into()),
        };
        result_detail(result, reader)
    }

    fn road(
        &mut self,
        grfid: u32,
        request: Request,
        reader: &mut Reader<'_>,
        loc: LoadLocation,
    ) -> Result<Value> {
        let file = self.registry.file(loc.file).ok_or("file")?;
        let input = super::super::super::load_cargo_road::Input {
            identity: FileIdentity {
                index: loc.file,
                grfid,
            },
            version: file.version,
            table: file.cargo.as_ref().ok_or("table")?,
            state: &self.cargo,
        };
        let mut resolutions = Vec::new();
        let result: ActionResult = self.specs.road_cargo(
            input,
            RoadRequest {
                grfid,
                first: u32::from(request.first),
                count: u32::from(request.count),
                property: request.property,
            },
            reader,
            &mut self.budget,
            loc,
            |state, index, remaining| {
                resolutions.push((state.clone(), index, remaining));
                Ok(())
            },
        );
        for (state, index, remaining) in resolutions {
            let local = state
                .owners
                .get(index)
                .ok_or("resolved owner")?
                .spec
                .local_id;
            self.check_specs(
                "road-owner-resolved",
                Some(&json!({"local_id":local,"engine":index,"remaining":remaining})),
                &state,
            )?;
        }
        result_detail(result.map(|()| Change::Success), reader)
    }

    pub(super) fn finish(&mut self) -> Result {
        self.check("api-finish", Some(&Value::Null))?;
        assert!(self.events.is_empty());
        Ok(())
    }
}
