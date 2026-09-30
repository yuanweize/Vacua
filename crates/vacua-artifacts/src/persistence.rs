use crate::ecosystem::DeveloperEcosystem;
use crate::error::Result;
use crate::evidence::{ActiveProjectState, RebuildConfidence, RebuildEvidence};
use crate::id::{DeveloperArtifactId, DeveloperProjectId};
use crate::kind::DeveloperArtifactKind;
use crate::model::{
    ArtifactAnalysisGeneration, DeveloperArtifact, DeveloperArtifactCoverage, DeveloperProject,
};
use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};

pub struct ArtifactPersistence {
    conn: Arc<Mutex<Connection>>,
}

impl ArtifactPersistence {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Save a newly scanned generation using instance connection
    pub fn save_generation(&self, gen: &ArtifactAnalysisGeneration) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        Self::publish_generation(&mut conn, gen)
    }

    /// Load the latest ready generation using instance connection
    pub fn get_latest_generation(
        &self,
        root_id: &str,
    ) -> Result<Option<ArtifactAnalysisGeneration>> {
        let conn = self.conn.lock().unwrap();
        Self::get_latest_ready_generation(&conn, root_id)
    }

    /// Query artifacts using instance connection
    #[allow(clippy::too_many_arguments)]
    pub fn query_artifacts_page(
        &self,
        generation_id: &str,
        ecosystem_filter: Option<DeveloperEcosystem>,
        kind_filter: Option<DeveloperArtifactKind>,
        confidence_filter: Option<RebuildConfidence>,
        project_id_filter: Option<&str>,
        min_allocated_bytes: Option<u64>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<DeveloperArtifact>> {
        let conn = self.conn.lock().unwrap();
        Self::query_artifacts(
            &conn,
            generation_id,
            ecosystem_filter,
            kind_filter,
            confidence_filter,
            project_id_filter,
            min_allocated_bytes,
            limit,
            offset,
        )
    }

    /// Get single artifact using instance connection
    pub fn get_single_artifact(&self, artifact_id: &str) -> Result<Option<DeveloperArtifact>> {
        let conn = self.conn.lock().unwrap();
        Self::get_artifact(&conn, artifact_id)
    }

    // --- Static methods taking direct &Connection or &mut Connection ---

    /// Atomically publishes a newly built generation into the SQLite index
    pub fn publish_generation(
        conn: &mut Connection,
        gen: &ArtifactAnalysisGeneration,
    ) -> Result<()> {
        let tx = conn.transaction()?;

        // 1. Insert generation record with status 'building'
        tx.execute(
            r#"
            INSERT INTO developer_artifact_generations (
                generation_id, root_path, root_id, observed_at, status,
                projects_count, artifacts_count, total_logical_bytes, total_allocated_bytes,
                coverage_json
            ) VALUES (?1, ?2, ?3, ?4, 'building', ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                gen.generation_id,
                gen.root_path,
                gen.root_id,
                gen.observed_at,
                gen.projects.len() as i64,
                gen.projects
                    .iter()
                    .map(|p| p.artifacts.len() as i64)
                    .sum::<i64>(),
                gen.total_logical_bytes as i64,
                gen.total_allocated_bytes as i64,
                serde_json::to_string(&gen.coverage)?,
            ],
        )?;

        // 2. Insert projects and artifacts
        for proj in &gen.projects {
            let all_eco_json = serde_json::to_string(&proj.all_ecosystems)?;
            let manifests_json = serde_json::to_string(&proj.manifest_paths)?;
            let lockfiles_json = serde_json::to_string(&proj.lockfile_paths)?;

            tx.execute(
                r#"
                INSERT INTO developer_projects (
                    generation_id, project_id, root_relative_path, display_name, display_path,
                    primary_ecosystem, ecosystems_json, manifest_paths_json, lockfile_paths_json,
                    artifacts_count, total_logical_bytes, total_allocated_bytes,
                    rebuild_confidence, active_state
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                "#,
                params![
                    gen.generation_id,
                    proj.project_id.as_str(),
                    proj.raw_relative_path,
                    proj.display_name,
                    proj.display_path,
                    proj.primary_ecosystem.as_str(),
                    all_eco_json,
                    manifests_json,
                    lockfiles_json,
                    proj.artifacts.len() as i64,
                    proj.total_logical_bytes as i64,
                    proj.total_allocated_bytes as i64,
                    proj.rebuild_confidence.as_str(),
                    proj.active_state.as_str(),
                ],
            )?;

            for art in &proj.artifacts {
                let evidence_json = serde_json::to_string(&art.rebuild_evidence)?;

                tx.execute(
                    r#"
                    INSERT INTO developer_artifacts (
                        generation_id, artifact_id, project_id, root_relative_path, display_name,
                        display_path, ecosystem, artifact_kind, logical_bytes, allocated_bytes,
                        confirmed_reclaim_lower_bound, rebuild_confidence, evidence_json, candidate_id
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                    "#,
                    params![
                        gen.generation_id,
                        art.artifact_id.as_str(),
                        proj.project_id.as_str(),
                        art.raw_relative_path,
                        art.display_name,
                        art.display_path,
                        art.ecosystem.as_str(),
                        art.artifact_kind.as_str(),
                        art.logical_bytes as i64,
                        art.allocated_bytes as i64,
                        art.confirmed_reclaim_lower_bound as i64,
                        art.rebuild_evidence.reconstruction_confidence.as_str(),
                        evidence_json,
                        art.candidate_id,
                    ],
                )?;
            }
        }

        // 3. Transition generation status atomically to 'ready'
        tx.execute(
            "UPDATE developer_artifact_generations SET status = 'ready' WHERE generation_id = ?1",
            params![gen.generation_id],
        )?;

        tx.commit()?;

        // 4. Prune older generations (retain latest 2 per root)
        let _ = Self::prune_older_generations(conn, &gen.root_id, 2);

        Ok(())
    }

    /// Load latest ready generation
    pub fn get_latest_ready_generation(
        conn: &Connection,
        root_id: &str,
    ) -> Result<Option<ArtifactAnalysisGeneration>> {
        let mut stmt = conn.prepare(
            r#"
            SELECT generation_id, root_path, root_id, observed_at, total_logical_bytes, total_allocated_bytes, coverage_json
            FROM developer_artifact_generations
            WHERE root_id = ?1 AND status = 'ready'
            ORDER BY observed_at DESC
            LIMIT 1
            "#,
        )?;

        let mut rows = stmt.query(params![root_id])?;
        if let Some(row) = rows.next()? {
            let gen_id: String = row.get(0)?;
            let root_path: String = row.get(1)?;
            let root_id: String = row.get(2)?;
            let observed_at: i64 = row.get(3)?;
            let total_logical: i64 = row.get(4)?;
            let total_allocated: i64 = row.get(5)?;
            let coverage_json: String = row.get(6)?;

            let coverage: DeveloperArtifactCoverage = serde_json::from_str(&coverage_json)
                .unwrap_or(DeveloperArtifactCoverage {
                    supported_ecosystems: Vec::new(),
                    unclassified_candidate_directories: 0,
                    skipped_items: 0,
                });

            let projects = Self::load_projects_for_generation(conn, &gen_id)?;

            Ok(Some(ArtifactAnalysisGeneration {
                generation_id: gen_id,
                root_id,
                root_path,
                observed_at,
                projects,
                total_logical_bytes: total_logical as u64,
                total_allocated_bytes: total_allocated as u64,
                coverage,
            }))
        } else {
            Ok(None)
        }
    }

    /// Query artifacts with deterministic filtering and pagination
    #[allow(clippy::too_many_arguments)]
    pub fn query_artifacts(
        conn: &Connection,
        generation_id: &str,
        ecosystem_filter: Option<DeveloperEcosystem>,
        kind_filter: Option<DeveloperArtifactKind>,
        confidence_filter: Option<RebuildConfidence>,
        project_id_filter: Option<&str>,
        min_allocated_bytes: Option<u64>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<DeveloperArtifact>> {
        let bounded_limit = limit.clamp(1, 500);

        let mut query = String::from(
            r#"
            SELECT artifact_id, project_id, root_relative_path, display_name, display_path,
                   ecosystem, artifact_kind, logical_bytes, allocated_bytes,
                   confirmed_reclaim_lower_bound, evidence_json, candidate_id
            FROM developer_artifacts
            WHERE generation_id = ?1
            "#,
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(generation_id.to_string())];

        if let Some(eco) = ecosystem_filter {
            query.push_str(" AND ecosystem = ?");
            params.push(Box::new(eco.as_str().to_string()));
        }

        if let Some(k) = kind_filter {
            query.push_str(" AND artifact_kind = ?");
            params.push(Box::new(k.as_str().to_string()));
        }

        if let Some(conf) = confidence_filter {
            query.push_str(" AND rebuild_confidence = ?");
            params.push(Box::new(conf.as_str().to_string()));
        }

        if let Some(pid) = project_id_filter {
            query.push_str(" AND project_id = ?");
            params.push(Box::new(pid.to_string()));
        }

        if let Some(min_bytes) = min_allocated_bytes {
            query.push_str(" AND allocated_bytes >= ?");
            params.push(Box::new(min_bytes as i64));
        }

        query.push_str(" ORDER BY allocated_bytes DESC, artifact_id ASC LIMIT ? OFFSET ?");
        params.push(Box::new(bounded_limit as i64));
        params.push(Box::new(offset as i64));

        let mut stmt = conn.prepare(&query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let art_id: String = row.get(0)?;
            let proj_id: String = row.get(1)?;
            let raw_rel: Vec<u8> = row.get(2)?;
            let display_name: String = row.get(3)?;
            let display_path: String = row.get(4)?;
            let eco_str: String = row.get(5)?;
            let kind_str: String = row.get(6)?;
            let logical: i64 = row.get(7)?;
            let allocated: i64 = row.get(8)?;
            let lower_bound: i64 = row.get(9)?;
            let evidence_json: String = row.get(10)?;
            let candidate_id: Option<String> = row.get(11)?;

            let ecosystem =
                DeveloperEcosystem::from_str_name(&eco_str).unwrap_or(DeveloperEcosystem::Unknown);
            let artifact_kind = DeveloperArtifactKind::from_str_name(&kind_str)
                .unwrap_or(DeveloperArtifactKind::OtherGenerated);
            let rebuild_evidence: RebuildEvidence =
                serde_json::from_str(&evidence_json).unwrap_or_default();

            Ok(DeveloperArtifact {
                artifact_id: DeveloperArtifactId(art_id),
                project_id: DeveloperProjectId(proj_id),
                raw_relative_path: raw_rel,
                display_name,
                display_path,
                ecosystem,
                artifact_kind,
                logical_bytes: logical as u64,
                allocated_bytes: allocated as u64,
                confirmed_reclaim_lower_bound: lower_bound as u64,
                estimated_reclaim: allocated as u64,
                physical_sharing_uncertainty: true,
                rebuild_evidence,
                candidate_id,
            })
        })?;

        let mut artifacts = Vec::new();
        for r in rows {
            artifacts.push(r?);
        }

        Ok(artifacts)
    }

    /// Retrieve detailed view for a single artifact
    pub fn get_artifact(conn: &Connection, artifact_id: &str) -> Result<Option<DeveloperArtifact>> {
        let mut stmt = conn.prepare(
            r#"
            SELECT artifact_id, project_id, root_relative_path, display_name, display_path,
                   ecosystem, artifact_kind, logical_bytes, allocated_bytes,
                   confirmed_reclaim_lower_bound, evidence_json, candidate_id
            FROM developer_artifacts
            WHERE artifact_id = ?1
            LIMIT 1
            "#,
        )?;

        let mut rows = stmt.query(params![artifact_id])?;
        if let Some(row) = rows.next()? {
            let art_id: String = row.get(0)?;
            let proj_id: String = row.get(1)?;
            let raw_rel: Vec<u8> = row.get(2)?;
            let display_name: String = row.get(3)?;
            let display_path: String = row.get(4)?;
            let eco_str: String = row.get(5)?;
            let kind_str: String = row.get(6)?;
            let logical: i64 = row.get(7)?;
            let allocated: i64 = row.get(8)?;
            let lower_bound: i64 = row.get(9)?;
            let evidence_json: String = row.get(10)?;
            let candidate_id: Option<String> = row.get(11)?;

            let ecosystem =
                DeveloperEcosystem::from_str_name(&eco_str).unwrap_or(DeveloperEcosystem::Unknown);
            let artifact_kind = DeveloperArtifactKind::from_str_name(&kind_str)
                .unwrap_or(DeveloperArtifactKind::OtherGenerated);
            let rebuild_evidence: RebuildEvidence =
                serde_json::from_str(&evidence_json).unwrap_or_default();

            Ok(Some(DeveloperArtifact {
                artifact_id: DeveloperArtifactId(art_id),
                project_id: DeveloperProjectId(proj_id),
                raw_relative_path: raw_rel,
                display_name,
                display_path,
                ecosystem,
                artifact_kind,
                logical_bytes: logical as u64,
                allocated_bytes: allocated as u64,
                confirmed_reclaim_lower_bound: lower_bound as u64,
                estimated_reclaim: allocated as u64,
                physical_sharing_uncertainty: true,
                rebuild_evidence,
                candidate_id,
            }))
        } else {
            Ok(None)
        }
    }

    fn load_projects_for_generation(
        conn: &Connection,
        gen_id: &str,
    ) -> Result<Vec<DeveloperProject>> {
        let mut stmt = conn.prepare(
            r#"
            SELECT project_id, root_relative_path, display_name, display_path,
                   primary_ecosystem, ecosystems_json, manifest_paths_json, lockfile_paths_json,
                   total_logical_bytes, total_allocated_bytes, rebuild_confidence, active_state
            FROM developer_projects
            WHERE generation_id = ?1
            ORDER BY total_allocated_bytes DESC, display_name ASC
            "#,
        )?;

        let proj_rows = stmt.query_map(params![gen_id], |row| {
            let pid: String = row.get(0)?;
            let raw_rel: Vec<u8> = row.get(1)?;
            let display_name: String = row.get(2)?;
            let display_path: String = row.get(3)?;
            let primary_eco_str: String = row.get(4)?;
            let all_eco_json: String = row.get(5)?;
            let manifests_json: String = row.get(6)?;
            let lockfiles_json: String = row.get(7)?;
            let logical: i64 = row.get(8)?;
            let allocated: i64 = row.get(9)?;
            let conf_str: String = row.get(10)?;
            let active_str: String = row.get(11)?;

            let primary_ecosystem = DeveloperEcosystem::from_str_name(&primary_eco_str)
                .unwrap_or(DeveloperEcosystem::Unknown);
            let all_ecosystems: Vec<DeveloperEcosystem> =
                serde_json::from_str(&all_eco_json).unwrap_or_default();
            let manifest_paths: Vec<String> =
                serde_json::from_str(&manifests_json).unwrap_or_default();
            let lockfile_paths: Vec<String> =
                serde_json::from_str(&lockfiles_json).unwrap_or_default();
            let rebuild_confidence =
                RebuildConfidence::from_str_name(&conf_str).unwrap_or(RebuildConfidence::Unknown);
            let active_state = match active_str.as_str() {
                "active" => ActiveProjectState::Active,
                "dormant" => ActiveProjectState::Dormant,
                _ => ActiveProjectState::Unknown,
            };

            Ok((
                pid,
                raw_rel,
                display_name,
                display_path,
                primary_ecosystem,
                all_ecosystems,
                manifest_paths,
                lockfile_paths,
                logical,
                allocated,
                rebuild_confidence,
                active_state,
            ))
        })?;

        let mut projects = Vec::new();
        for r in proj_rows {
            let (
                pid,
                raw_rel,
                display_name,
                display_path,
                primary_ecosystem,
                all_ecosystems,
                manifest_paths,
                lockfile_paths,
                logical,
                allocated,
                rebuild_confidence,
                active_state,
            ) = r?;

            let artifacts = Self::load_artifacts_for_project(conn, gen_id, &pid)?;

            projects.push(DeveloperProject {
                project_id: DeveloperProjectId(pid),
                raw_relative_path: raw_rel,
                display_name,
                display_path,
                primary_ecosystem,
                all_ecosystems,
                manifest_paths,
                lockfile_paths,
                artifacts,
                total_logical_bytes: logical as u64,
                total_allocated_bytes: allocated as u64,
                rebuild_confidence,
                active_state,
            });
        }

        Ok(projects)
    }

    fn load_artifacts_for_project(
        conn: &Connection,
        gen_id: &str,
        project_id: &str,
    ) -> Result<Vec<DeveloperArtifact>> {
        let mut stmt = conn.prepare(
            r#"
            SELECT artifact_id, project_id, root_relative_path, display_name, display_path,
                   ecosystem, artifact_kind, logical_bytes, allocated_bytes,
                   confirmed_reclaim_lower_bound, evidence_json, candidate_id
            FROM developer_artifacts
            WHERE generation_id = ?1 AND project_id = ?2
            ORDER BY allocated_bytes DESC, artifact_id ASC
            "#,
        )?;

        let rows = stmt.query_map(params![gen_id, project_id], |row| {
            let art_id: String = row.get(0)?;
            let proj_id: String = row.get(1)?;
            let raw_rel: Vec<u8> = row.get(2)?;
            let display_name: String = row.get(3)?;
            let display_path: String = row.get(4)?;
            let eco_str: String = row.get(5)?;
            let kind_str: String = row.get(6)?;
            let logical: i64 = row.get(7)?;
            let allocated: i64 = row.get(8)?;
            let lower_bound: i64 = row.get(9)?;
            let evidence_json: String = row.get(10)?;
            let candidate_id: Option<String> = row.get(11)?;

            let ecosystem =
                DeveloperEcosystem::from_str_name(&eco_str).unwrap_or(DeveloperEcosystem::Unknown);
            let artifact_kind = DeveloperArtifactKind::from_str_name(&kind_str)
                .unwrap_or(DeveloperArtifactKind::OtherGenerated);
            let rebuild_evidence: RebuildEvidence =
                serde_json::from_str(&evidence_json).unwrap_or_default();

            Ok(DeveloperArtifact {
                artifact_id: DeveloperArtifactId(art_id),
                project_id: DeveloperProjectId(proj_id),
                raw_relative_path: raw_rel,
                display_name,
                display_path,
                ecosystem,
                artifact_kind,
                logical_bytes: logical as u64,
                allocated_bytes: allocated as u64,
                confirmed_reclaim_lower_bound: lower_bound as u64,
                estimated_reclaim: allocated as u64,
                physical_sharing_uncertainty: true,
                rebuild_evidence,
                candidate_id,
            })
        })?;

        let mut artifacts = Vec::new();
        for r in rows {
            artifacts.push(r?);
        }
        Ok(artifacts)
    }

    pub fn prune_older_generations(
        conn: &mut Connection,
        root_id: &str,
        keep: usize,
    ) -> Result<()> {
        let mut stmt = conn.prepare(
            r#"
            SELECT generation_id
            FROM developer_artifact_generations
            WHERE root_id = ?1 AND status = 'ready'
            ORDER BY observed_at DESC
            LIMIT -1 OFFSET ?2
            "#,
        )?;

        let old_gen_ids: Vec<String> = stmt
            .query_map(params![root_id, keep as i64], |row| row.get(0))?
            .filter_map(std::result::Result::ok)
            .collect();

        for old_id in old_gen_ids {
            conn.execute(
                "DELETE FROM developer_artifacts WHERE generation_id = ?1",
                params![old_id],
            )?;
            conn.execute(
                "DELETE FROM developer_projects WHERE generation_id = ?1",
                params![old_id],
            )?;
            conn.execute(
                "DELETE FROM developer_artifact_generations WHERE generation_id = ?1",
                params![old_id],
            )?;
        }

        Ok(())
    }
}
