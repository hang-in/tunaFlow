import { cn } from "@/lib/utils";
import { FolderOpen, Folder } from "lucide-react";
import { TreeRow } from "./TreeRow";
import type { Project } from "@/types";

interface ProjectsSectionProps {
  projects: Project[];
  selectedProjectKey: string | null;
  selectProject: (key: string) => void;
}

export function ProjectsSection({ projects, selectedProjectKey, selectProject }: ProjectsSectionProps) {
  const currentProject = projects.find((p) => p.key === selectedProjectKey);

  return (
    <>
      <div className="px-2 mt-1">
        <span className="text-[10px] font-semibold uppercase tracking-wider text-sidebar-foreground/50 pl-1">Projects</span>
      </div>
      <div className="mt-0.5">
        {projects.map((project) => {
          const isSelected = project.key === selectedProjectKey;
          return (
            <TreeRow key={project.key} depth={0} active={isSelected}
              icon={isSelected
                ? <FolderOpen className="w-3.5 h-3.5 text-primary" />
                : <Folder className="w-3.5 h-3.5 text-sidebar-foreground/35" />}
              label={<span className={cn("truncate", isSelected && "font-medium")}>{project.name}</span>}
              suffix={isSelected && currentProject?.path ? (
                <span className="text-[8px] text-sidebar-foreground/25 truncate max-w-[60px] shrink-0 mr-1" title={currentProject.path}>
                  {currentProject.path.split(/[\\/]/).pop()}
                </span>
              ) : undefined}
              onClick={() => selectProject(project.key)} />
          );
        })}
      </div>
      <div className="mx-2 mt-2 mb-1 border-t border-white/[0.06]" />
    </>
  );
}
