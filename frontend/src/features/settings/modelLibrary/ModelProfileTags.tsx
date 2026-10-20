import type { ProviderProfileResponse } from "../../../bridge/desktop";
import { profileModelType } from "../../../utils/providerModel";
import { MODEL_TYPE_LABELS } from "./modelLibraryUtils";

export default function ModelProfileTags({
  profile,
  showType = true,
}: {
  profile: Pick<ProviderProfileResponse, "model_type" | "kind" | "model_id" | "default_for" | "source" | "status">;
  showType?: boolean;
}) {
  const isDefault = (profile.default_for?.length ?? 0) > 0;
  const type = profileModelType(profile);
  return (
    <span className="suna-model-profile-tags">
      {isDefault && <span className="suna-model-tag-default">默认</span>}
      {profile.status === "downloading" && <span className="suna-model-tag-muted">下载中</span>}
      {profile.status === "download_failed" && <span className="suna-model-tag-muted">下载失败</span>}
      {profile.source === "local" && <span className="suna-model-tag-muted">本地</span>}
      {showType && <span className="suna-model-tag-type">{MODEL_TYPE_LABELS[type]}</span>}
    </span>
  );
}
