import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { getSetting, setSetting } from "@/lib/appStore";
import { User, Code2, GitBranch, KeyRound, Clock, Languages } from "lucide-react";
import { SUPPORTED_LOCALES, setLocale, getCurrentLocale, type SupportedLocale } from "@/locales";

export interface UserProfile {
  // Basic
  name: string;
  title: string;
  githubUsername: string;
  // Agent context
  bio: string;
  preferredLanguages: string;
  // Dev info
  gitName: string;
  gitEmail: string;
  githubOrg: string;
  // Advanced
  githubToken: string;
}

const DEFAULT_PROFILE: UserProfile = {
  name: "",
  title: "",
  githubUsername: "",
  bio: "",
  preferredLanguages: "",
  gitName: "",
  gitEmail: "",
  githubOrg: "",
  githubToken: "",
};

function FieldRow({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1">
      <label className="text-[12px] font-medium text-foreground/80">{label}</label>
      {children}
      {hint && <p className="text-[11px] text-muted-foreground/50">{hint}</p>}
    </div>
  );
}

function Input({
  value,
  onChange,
  placeholder,
  type = "text",
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  type?: string;
}) {
  return (
    <input
      type={type}
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      className="w-full bg-background border border-border/40 rounded-md px-3 py-1.5 text-[13px] text-foreground placeholder:text-muted-foreground/30 focus:outline-none focus:border-ring/50 transition-colors"
    />
  );
}

function SectionHeader({ icon, label }: { icon: React.ReactNode; label: string }) {
  return (
    <div className="flex items-center gap-2 mb-3">
      <span className="text-muted-foreground/50">{icon}</span>
      <span className="text-[12px] font-semibold text-muted-foreground/70 uppercase tracking-wide">{label}</span>
    </div>
  );
}

export function ProfileSection() {
  const [profile, setProfile] = useState<UserProfile>(DEFAULT_PROFILE);
  const [saved, setSaved] = useState(false);

  const timezone = Intl.DateTimeFormat().resolvedOptions().timeZone;
  const avatarUrl = profile.githubUsername
    ? `https://github.com/${profile.githubUsername}.png?size=80`
    : null;

  useEffect(() => {
    getSetting<UserProfile>("userProfile", DEFAULT_PROFILE).then((v) => {
      setProfile({ ...DEFAULT_PROFILE, ...v });
    });
  }, []);

  const update = (patch: Partial<UserProfile>) => {
    setProfile((prev) => ({ ...prev, ...patch }));
    setSaved(false);
  };

  const handleSave = () => {
    setSetting("userProfile", profile);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
    // Invalidate MessageMeta cache so new messages show updated name/avatar immediately
    window.dispatchEvent(new CustomEvent("tunaflow:profile-changed"));
  };

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-[14px] font-[550] text-foreground mb-1">Profile</h2>
        <p className="text-[12px] text-muted-foreground">에이전트 컨텍스트와 개발자 정보를 설정합니다.</p>
      </div>

      <LanguageSelector />

      {/* ── Basic ─────────────────────────────────────────────── */}
      <div className="space-y-3">
        <SectionHeader icon={<User className="w-3.5 h-3.5" />} label="기본 정보" />
        <div className="flex items-start gap-4">
          {/* Avatar */}
          <div className="shrink-0">
            {avatarUrl ? (
              <img
                src={avatarUrl}
                alt="avatar"
                className="w-14 h-14 rounded-full border border-border/30 object-cover"
                onError={(e) => { (e.target as HTMLImageElement).style.display = "none"; }}
              />
            ) : (
              <div className="w-14 h-14 rounded-full border border-border/30 bg-accent/30 flex items-center justify-center">
                <User className="w-6 h-6 text-muted-foreground/30" />
              </div>
            )}
          </div>
          <div className="flex-1 grid grid-cols-2 gap-3">
            <FieldRow label="이름">
              <Input value={profile.name} onChange={(v) => update({ name: v })} placeholder="Hong Gildong" />
            </FieldRow>
            <FieldRow label="직함">
              <Input value={profile.title} onChange={(v) => update({ title: v })} placeholder="Software Engineer" />
            </FieldRow>
            <FieldRow label="GitHub Username" hint="프로필 사진 자동 로드에 사용됩니다.">
              <Input value={profile.githubUsername} onChange={(v) => update({ githubUsername: v })} placeholder="octocat" />
            </FieldRow>
          </div>
        </div>
      </div>

      {/* ── Agent context ──────────────────────────────────────── */}
      <div className="space-y-3">
        <SectionHeader icon={<Code2 className="w-3.5 h-3.5" />} label="에이전트 컨텍스트" />
        <FieldRow label="소개 메모" hint="에이전트가 사용자 배경을 이해하는 데 활용됩니다.">
          <textarea
            value={profile.bio}
            onChange={(e) => update({ bio: e.target.value })}
            placeholder="백엔드 API 개발 주력, Rust/TypeScript 사용, 스타트업 CTO..."
            rows={3}
            className="w-full bg-background border border-border/40 rounded-md px-3 py-2 text-[13px] text-foreground placeholder:text-muted-foreground/30 focus:outline-none focus:border-ring/50 transition-colors resize-none"
          />
        </FieldRow>
        <FieldRow label="선호 언어" hint="쉼표로 구분합니다.">
          <Input value={profile.preferredLanguages} onChange={(v) => update({ preferredLanguages: v })} placeholder="TypeScript, Rust, Python" />
        </FieldRow>
      </div>

      {/* ── Dev info ───────────────────────────────────────────── */}
      <div className="space-y-3">
        <SectionHeader icon={<GitBranch className="w-3.5 h-3.5" />} label="개발자 정보" />
        <div className="grid grid-cols-2 gap-3">
          <FieldRow label="Git 이름">
            <Input value={profile.gitName} onChange={(v) => update({ gitName: v })} placeholder="Hong Gildong" />
          </FieldRow>
          <FieldRow label="Git 이메일">
            <Input value={profile.gitEmail} onChange={(v) => update({ gitEmail: v })} placeholder="user@example.com" />
          </FieldRow>
          <FieldRow label="GitHub 기본 Org">
            <Input value={profile.githubOrg} onChange={(v) => update({ githubOrg: v })} placeholder="my-org" />
          </FieldRow>
        </div>
      </div>

      {/* ── Timezone (auto) ────────────────────────────────────── */}
      <div className="space-y-3">
        <SectionHeader icon={<Clock className="w-3.5 h-3.5" />} label="시간대" />
        <div className="flex items-center gap-2 px-3 py-2 bg-accent/20 rounded-md border border-border/20">
          <span className="text-[13px] text-foreground/60">{timezone}</span>
          <span className="text-[11px] text-muted-foreground/40 ml-1">자동 감지</span>
        </div>
      </div>

      {/* ── Advanced (GitHub Token) ────────────────────────────── */}
      <div className="space-y-3">
        <SectionHeader icon={<KeyRound className="w-3.5 h-3.5" />} label="고급" />
        <FieldRow label="GitHub Personal Access Token" hint="레포 클론, API 접근 시 사용됩니다. 로컬에만 저장됩니다.">
          <Input
            type="password"
            value={profile.githubToken}
            onChange={(v) => update({ githubToken: v })}
            placeholder="ghp_..."
          />
        </FieldRow>
      </div>

      {/* Save button */}
      <div className="flex items-center gap-3 pt-2">
        <button
          onClick={handleSave}
          className="px-4 py-1.5 bg-primary text-primary-foreground text-[13px] font-medium rounded-md hover:bg-primary/90 transition-colors"
        >
          저장
        </button>
        {saved && (
          <span className="text-[12px] text-status-approved">저장됐습니다</span>
        )}
      </div>
    </div>
  );
}

/** i18nPlan Phase 1-4: Language selector. appStore 아닌 localStorage 경로 —
 *  i18next 의 LanguageDetector 가 'localStorage' 를 lookup 하도록 설정했기 때문. */
function LanguageSelector() {
  const { t } = useTranslation();
  const [locale, setLocaleState] = useState<SupportedLocale>(getCurrentLocale());

  const handleChange = async (next: SupportedLocale) => {
    setLocaleState(next);
    await setLocale(next);
  };

  return (
    <div className="space-y-3">
      <SectionHeader icon={<Languages className="w-3.5 h-3.5" />} label={t("language.label")} />
      <select
        value={locale}
        onChange={(e) => handleChange(e.target.value as SupportedLocale)}
        className="w-full bg-background border border-border/40 rounded-md px-3 py-1.5 text-[13px] text-foreground focus:outline-none focus:border-ring/50 transition-colors"
      >
        {SUPPORTED_LOCALES.map((loc) => (
          <option key={loc} value={loc}>
            {t(`language.${loc}` as "language.ko" | "language.en")}
          </option>
        ))}
      </select>
    </div>
  );
}
