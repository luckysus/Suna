import { Button } from "../../components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "../../components/ui/dialog";
import { Input } from "../../components/ui/input";
import type { ResearchProject } from "../../bridge/desktop";

export interface ResearchProjectEditorDialogProps {
  editing: ResearchProject | "new" | null;
  name: string;
  description: string;
  material: string;
  tags: string;
  setName: (val: string) => void;
  setDescription: (val: string) => void;
  setMaterial: (val: string) => void;
  setTags: (val: string) => void;
  onSave: () => void;
  onClose: () => void;
}

export default function ResearchProjectEditorDialog({
  editing,
  name,
  description,
  material,
  tags,
  setName,
  setDescription,
  setMaterial,
  setTags,
  onSave,
  onClose,
}: ResearchProjectEditorDialogProps) {
  return (
    <Dialog open={editing !== null} onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogContent className="suna-project-editor">
        <DialogTitle>{editing === "new" ? "新建科研项目" : "编辑科研项目"}</DialogTitle>
        <DialogDescription>填写科研目标，原始数据和模型通过关联添加。</DialogDescription>
        <label>
          项目名称
          <Input value={name} onChange={(e) => setName(e.target.value)} maxLength={160} />
        </label>
        <label>
          科研目标与备注
          <textarea value={description} onChange={(e) => setDescription(e.target.value)} />
        </label>
        <label>
          材料体系
          <Input value={material} onChange={(e) => setMaterial(e.target.value)} />
        </label>
        <label>
          标签（逗号分隔）
          <Input value={tags} onChange={(e) => setTags(e.target.value)} />
        </label>
        <div className="suna-project-editor-actions">
          <Button variant="action" disabled={!name.trim()} onClick={() => void onSave()}>
            保存项目
          </Button>
          <Button variant="ghost" onClick={onClose}>
            取消
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
