//! 内置默认 Skill（第 52 章）。
//!
//! 随应用发布 8 个文档要求的专业 Skill，安装/启动时**种子式写入**用户
//! Skill 目录（`~/.suna/skills`）：只创建缺失的文件，绝不覆盖用户已修改
//! 的内容。Skill 本体在编译期通过 `include_str!` 嵌入，不依赖运行期资源
//! 路径，开发与打包行为一致。

use std::fs;
use std::path::Path;

/// (目录名, SKILL.md 内容)。目录名与 frontmatter 里的 name 一致。
const BUNDLED_SKILLS: &[(&str, &str)] = &[
    (
        "wear-resistant-steel-analysis",
        include_str!("../../resources/skills/wear-resistant-steel-analysis/SKILL.md"),
    ),
    (
        "if-steel-analysis",
        include_str!("../../resources/skills/if-steel-analysis/SKILL.md"),
    ),
    (
        "paper-summary",
        include_str!("../../resources/skills/paper-summary/SKILL.md"),
    ),
    (
        "data-analysis",
        include_str!("../../resources/skills/data-analysis/SKILL.md"),
    ),
    (
        "performance-prediction",
        include_str!("../../resources/skills/performance-prediction/SKILL.md"),
    ),
    (
        "process-optimization",
        include_str!("../../resources/skills/process-optimization/SKILL.md"),
    ),
    (
        "experiment-design",
        include_str!("../../resources/skills/experiment-design/SKILL.md"),
    ),
    (
        "metallographic-analysis",
        include_str!("../../resources/skills/metallographic-analysis/SKILL.md"),
    ),
];

pub const BUNDLED_SKILL_NAMES: &[&str] = &[
    "wear-resistant-steel-analysis",
    "if-steel-analysis",
    "paper-summary",
    "data-analysis",
    "performance-prediction",
    "process-optimization",
    "experiment-design",
    "metallographic-analysis",
];

/// 把内置 Skill 种子到 `root`，返回新写入的文件数。已存在的文件一律不动。
pub fn seed_bundled_skills(root: &Path) -> std::io::Result<usize> {
    let mut seeded = 0;
    for (name, content) in BUNDLED_SKILLS {
        let skill_file = root.join(name).join("SKILL.md");
        if skill_file.is_file() {
            continue;
        }
        fs::create_dir_all(skill_file.parent().expect("skill file has a parent"))?;
        fs::write(&skill_file, content)?;
        seeded += 1;
    }
    Ok(seeded)
}

/// 用户 Skill 目录（与 `skills::default_skill_roots` 的 User 根一致）。
pub fn user_skills_root() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".suna").join("skills"))
}

/// 种子到用户目录；home 不可用时返回 None（调用方记录日志即可）。
pub fn seed_bundled_user_skills() -> Option<std::io::Result<usize>> {
    user_skills_root().map(|root| seed_bundled_skills(&root))
}

#[cfg(test)]
mod tests {
    use super::{seed_bundled_skills, BUNDLED_SKILLS};
    use crate::skills::{discover_skills, SkillRoot, SkillScope};
    use std::fs;

    #[test]
    fn seeds_eight_default_skills_that_discover_cleanly() {
        let root = std::env::temp_dir().join(format!("suna-skills-{}", uuid::Uuid::new_v4()));
        let seeded = seed_bundled_skills(&root).expect("seed bundled skills");
        assert_eq!(seeded, BUNDLED_SKILLS.len());
        assert_eq!(seeded, 8);

        let report = discover_skills(&[SkillRoot::new(SkillScope::User, root.clone())], "1.0.0");
        assert!(
            report.errors.is_empty(),
            "seeded skills must parse: {:?}",
            report.errors
        );
        assert_eq!(report.skills.len(), 8);
        for name in super::BUNDLED_SKILL_NAMES {
            assert!(
                report.skills.iter().any(|skill| &skill.name == name),
                "missing bundled skill {name}"
            );
        }
        // 中文描述与正文写入为 UTF-8。
        let wear = report
            .skills
            .iter()
            .find(|skill| skill.name == "wear-resistant-steel-analysis")
            .expect("wear-resistant skill");
        assert!(wear.description.contains("耐磨钢"));
    }

    #[test]
    fn seeding_is_idempotent_and_never_overwrites_user_edits() {
        let root = std::env::temp_dir().join(format!("suna-skills-{}", uuid::Uuid::new_v4()));
        seed_bundled_skills(&root).expect("first seed");
        let skill_file = root.join("data-analysis").join("SKILL.md");
        fs::write(&skill_file, "用户自定义内容").expect("modify seeded skill");

        let seeded_again = seed_bundled_skills(&root).expect("second seed");
        assert_eq!(seeded_again, 0);
        assert_eq!(
            fs::read_to_string(&skill_file).expect("read modified skill"),
            "用户自定义内容"
        );
    }
}
