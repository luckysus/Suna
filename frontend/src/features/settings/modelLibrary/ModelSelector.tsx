import { Plus } from "lucide-react";
import type { ProviderProfileResponse, ProfileModelType } from "../../../bridge/desktop";
import { profileModelType } from "../../../utils/providerModel";
import { openModelLibrary } from "./modelLibraryEvents";
import ModelProfileTags from "./ModelProfileTags";

export default function ModelSelector({
  profiles,
  activeId,
  filterType = "chat",
  onSelect,
}: {
  profiles: ProviderProfileResponse[];
  activeId: string | null;
  filterType?: ProfileModelType;
  onSelect: (profileId: string) => void;
}) {
  const filtered = profiles.filter(
    (profile) => profile.enabled && profileModelType(profile) === filterType,
  );

  return (
    <div className="suna-chat-model-menu suna-model-selector-menu" role="menu">
      {filtered.map((profile) => {
        return (
          <button
            key={profile.id}
            type="button"
            role="menuitem"
            aria-label={profile.model_id || profile.display_name}
            className={`suna-model-selector-item ${profile.id === activeId ? "is-active" : ""}`}
            onClick={() => onSelect(profile.id)}
          >
            <span className="suna-model-selector-name">{profile.model_id || profile.display_name}</span>
            <span className="suna-model-selector-meta">
              <ModelProfileTags profile={profile} />
            </span>
          </button>
        );
      })}
      <button
        type="button"
        role="menuitem"
        className="suna-model-selector-add"
        onClick={() => openModelLibrary({ modelType: filterType })}
      >
        <Plus size={15} />
        添加模型
      </button>
    </div>
  );
}
