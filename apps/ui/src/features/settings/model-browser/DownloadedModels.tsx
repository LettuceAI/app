import { useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import {
  Copy,
  Cpu,
  FolderOpen,
  Image as ImageIcon,
  MoreHorizontal,
  Plus,
  Trash2,
  Zap,
} from "lucide-react";
import {
  Button,
  confirm,
  EmptyState,
  icon,
  IconButton,
  ListGroup,
  ListRow,
  LoadingState,
  Menu,
  MenuContent,
  MenuItem,
  MenuSeparator,
  MenuTrigger,
  relativeTime,
  SearchField,
  Table,
  Text,
  toast,
  Tooltip,
} from "@lettuceai/crisp";
import type { LocalModelFile, ProviderAccountView } from "@/api/generated/bindings";
import { revealLocalModel } from "@/api/local-model-files";
import { queryKeys } from "@/api/query-keys";
import { minimalModelConfig } from "@/entities/model";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsAction, useSettingsApi, useSettingsLayout } from "../data";
import { useByteSize } from "../image/local-data";
import { FolderMoves } from "./FolderMoves";
import { useFolderMoves } from "./folder-move-data";
import { browserKey, separator } from "./data";

export function DownloadedModels({ account }: { account: ProviderAccountView | undefined }) {
  return account ? <OllamaDownloaded key={account.id} account={account} /> : <LocalDownloaded />;
}
function LocalDownloaded() {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const navigate = useNavigate();
  const client = useQueryClient();
  const action = useSettingsAction();
  const size = useByteSize();
  const { phone } = useSettingsLayout();
  const [search, setSearch] = useState("");
  const createOperations = useRef(new Map<string, string>());
  const providerOperation = useRef(crypto.randomUUID());
  const files = useQuery({
    queryKey: queryKeys.localModels.all,
    queryFn: () => api.call("localModelsList"),
  });
  const dir = useQuery({
    queryKey: [...queryKeys.localModels.all, "directory"],
    queryFn: () => api.call("localModelsDirGet"),
  });
  const folderMoves = useFolderMoves();
  const rows =
    files.data?.files.filter((file) =>
      `${file.filename} ${file.repo} ${file.architecture ?? ""}`
        .toLowerCase()
        .includes(search.trim().toLowerCase()),
    ) ?? [];
  const refresh = async () => {
    await Promise.all([
      client.invalidateQueries({ queryKey: queryKeys.localModels.all }),
      client.invalidateQueries({ queryKey: browserKey }),
    ]);
  };
  const deleteFile = async (file: LocalModelFile) => {
    const names = file.used_by.map((reference) => reference.display_name ?? t("hf.globalDefaults"));
    if (
      !(await confirm({
        title: t("hf.deleteTitle"),
        message: [
          t("hf.deleteBody", { name: file.filename }),
          names.length ? t("hf.deleteUsed", { models: names.join(", ") }) : null,
        ]
          .filter(Boolean)
          .join(" "),
        confirmLabel: t("hf.delete"),
        cancelLabel: t("manage.cancel"),
        destructive: true,
      }))
    )
      return;
    const result = await api.call("localModelDelete", { path: file.path });
    if (result.referencing_profiles.length > 0)
      toast.warning(
        t("hf.deleteUsed", {
          models: result.referencing_profiles
            .map((reference) => reference.display_name ?? t("hf.globalDefaults"))
            .join(", "),
        }),
      );
    await refresh();
  };
  const makeModel = async (file: LocalModelFile) => {
    const currentAccounts = await client.fetchQuery({
      queryKey: queryKeys.providers.accounts(),
      queryFn: () => api.call("providerAccountsList"),
      staleTime: 0,
    });
    let localAccount =
      currentAccounts.find((item) => item.provider_kind === "llamacpp" && item.enabled) ??
      currentAccounts.find((item) => item.provider_kind === "llamacpp");
    if (!localAccount?.enabled) {
      const existing = localAccount;
      localAccount = await api.call("providerAccountSave", {
        account: {
          id: existing?.id ?? null,
          provider_kind: "llamacpp",
          label: existing?.label ?? t("hf.localProvider"),
          base_url: existing?.base_url ?? null,
          enabled: true,
          streaming_enabled: existing?.streaming_enabled ?? true,
          allow_invalid_tls: existing?.allow_invalid_tls ?? false,
          config: existing?.config ?? { kind: "standard" },
        },
        api_key: null,
        clear_api_key: false,
        expected_revision: existing?.revision ?? null,
        client_operation_id: providerOperation.current,
      });
      const saved = localAccount;
      client.setQueryData<ProviderAccountView[]>(queryKeys.providers.accounts(), (current) => [
        ...(current ?? []).filter((account) => account.id !== saved.id),
        saved,
      ]);
    }
    const operation = createOperations.current.get(file.path) ?? crypto.randomUUID();
    createOperations.current.set(file.path, operation);
    const created = await api.call("modelSave", {
      model: {
        id: null,
        provider_account_id: localAccount.id,
        external_model_id: file.path,
        display_name: file.filename.replace(/\.gguf$/i, ""),
        kind: "chat",
        config: minimalModelConfig,
        input_scopes: ["text"],
        output_scopes: ["text"],
        remote_metadata: null,
      },
      expected_revision: null,
      client_operation_id: operation,
    });
    await navigate({
      to: "/settings/models/$modelId",
      params: { modelId: created.id },
    });
  };
  const move = () =>
    action.mutate(async () => {
      const chosen = await api.call("filesPickDirectory");
      if (!chosen.path || chosen.path === dir.data?.path) return;
      if (
        !(await confirm({
          title: t("hf.folder"),
          message: t("hf.folderBody"),
          confirmLabel: t("hf.move"),
          cancelLabel: t("manage.cancel"),
        }))
      )
        return;
      const accepted = await api.call("localModelsDirSet", {
        path: chosen.path,
        move_existing: true,
        client_operation_id: crypto.randomUUID(),
      });
      folderMoves.admit(accepted.job_id);
      await refresh();
    });
  const menus = (file: LocalModelFile) => {
    const models = file.used_by.filter((reference) => reference.model_profile_id);
    return (
      <Menu>
        <MenuTrigger>
          <IconButton label={file.filename} size="sm">
            <MoreHorizontal size={icon.sm} />
          </IconButton>
        </MenuTrigger>
        <MenuContent align="end" className="w-60">
          {models.map((reference) => (
            <MenuItem
              key={reference.model_profile_id}
              icon={<Cpu size={icon.md} />}
              onSelect={() =>
                void navigate({
                  to: "/settings/models/$modelId",
                  params: { modelId: reference.model_profile_id! },
                })
              }
            >
              {t("hf.openNamed", { name: reference.display_name ?? t("hf.globalDefaults") })}
            </MenuItem>
          ))}
          {models.length === 0 && !file.is_mmproj && !file.is_mtp && !file.is_dflash && (
            <MenuItem
              icon={<Plus size={icon.md} />}
              onSelect={() => action.mutate(() => makeModel(file))}
            >
              {t("hf.makeModel")}
            </MenuItem>
          )}
          <MenuItem
            icon={<Copy size={icon.md} />}
            onSelect={() =>
              action.mutate(async () => {
                await navigator.clipboard.writeText(file.path);
                toast.success(t("hf.copied"));
              })
            }
          >
            {t("hf.copyPath")}
          </MenuItem>
          <MenuItem
            icon={<FolderOpen size={icon.md} />}
            onSelect={() => action.mutate(() => revealLocalModel(file.path))}
          >
            {t("hf.showFolder")}
          </MenuItem>
          <MenuSeparator />
          <MenuItem
            danger
            icon={<Trash2 size={icon.md} />}
            onSelect={() => action.mutate(() => deleteFile(file))}
          >
            {t("hf.deleteFile")}
          </MenuItem>
        </MenuContent>
      </Menu>
    );
  };
  const kindLabel = (file: LocalModelFile) =>
    file.is_mmproj
      ? t("hf.visionKind")
      : file.is_mtp
        ? t("hf.mtpKind")
        : file.is_dflash
          ? t("hf.dflashKind")
          : file.quantization;
  const usedBy = (file: LocalModelFile) =>
    file.used_by.map((item) => item.display_name ?? t("hf.globalDefaults")).join(", ");
  const total = files.data?.files.reduce((sum, file) => sum + file.size, 0) ?? 0;
  return (
    <div className="space-y-3">
      <div className={phone ? "flex flex-col gap-2" : "flex items-center gap-2"}>
        <div className="min-w-0 flex-1">
          <SearchField
            size="sm"
            value={search}
            onChange={setSearch}
            aria-label={t("hf.localSearch")}
            placeholder={t("hf.localSearch")}
          />
        </div>
        <div className="flex items-center gap-2">
          <Tooltip content={t("hf.folderTooltip")}>
            <Button
              size="sm"
              variant="secondary"
              leading={<FolderOpen size={icon.sm} />}
              disabled={dir.isPending || dir.isError || action.isPending || folderMoves.active}
              onClick={move}
            >
              <span className="max-w-64 truncate font-mono text-xs">{dir.data?.path ?? ""}</span>
              <span className="text-fg-3">{[separator, size(total)].join(" ")}</span>
            </Button>
          </Tooltip>
          <Button
            size="sm"
            variant="secondary"
            leading={<Plus size={icon.sm} />}
            loading={action.isPending}
            onClick={() =>
              action.mutate(async () => {
                const picked = await api.call("filesPickOpen", {
                  kinds: ["gguf_model"],
                  multiple: true,
                });
                for (const source of picked.sources)
                  await api.call("localModelAdopt", { source, model_name: null });
                await refresh();
              })
            }
          >
            {t("hf.addFile")}
          </Button>
        </div>
      </div>
      {dir.isError && <FailureState error={dir.error} onRetry={() => void dir.refetch()} />}
      <FolderMoves state={folderMoves} />
      {files.isPending ? (
        <LoadingState />
      ) : files.isError ? (
        <FailureState error={files.error} onRetry={() => void files.refetch()} />
      ) : rows.length === 0 ? (
        <EmptyState title={t("hf.noDownloads")} description={t("hf.noDownloadsBody")} />
      ) : phone ? (
        <ListGroup>
          {rows.map((file) => (
            <ListRow
              key={file.path}
              title={<span className="block truncate font-mono text-sm">{file.filename}</span>}
              description={
                file.used_by.length
                  ? t("hf.usedByLine", {
                      kind: kindLabel(file),
                      size: size(file.size),
                      models: usedBy(file),
                    })
                  : t("hf.notUsedLine", { kind: kindLabel(file), size: size(file.size) })
              }
              trailing={menus(file)}
            />
          ))}
        </ListGroup>
      ) : (
        <Table
          rows={rows}
          rowKey={(file) => file.path}
          columns={[
            {
              key: "file",
              header: t("hf.file"),
              cell: (file) => (
                <div className="min-w-0">
                  <Text className="block max-w-80 truncate font-mono text-sm" title={file.path}>
                    {file.filename}
                  </Text>
                  <Text variant="hint" className="block truncate text-xs">
                    {file.repo}
                  </Text>
                </div>
              ),
            },
            {
              key: "kind",
              header: t("hf.kind"),
              cell: (file) =>
                file.is_mmproj ? (
                  <span className="flex items-center gap-1.5">
                    <ImageIcon size={icon.sm} />
                    {kindLabel(file)}
                  </span>
                ) : file.is_mtp ? (
                  <span className="flex items-center gap-1.5">
                    <Zap size={icon.sm} />
                    {kindLabel(file)}
                  </span>
                ) : (
                  <span className="font-mono text-xs">{kindLabel(file)}</span>
                ),
            },
            {
              key: "arch",
              header: t("hf.arch"),
              cell: (file) => (
                <span className="font-mono text-xs text-fg-3">
                  {file.architecture ?? t("hf.unknown")}
                </span>
              ),
            },
            {
              key: "size",
              header: t("hf.size"),
              numeric: true,
              cell: (file) => size(file.size),
            },
            {
              key: "used",
              header: t("hf.usedBy"),
              cell: (file) =>
                file.used_by.length ? (
                  <span className="block max-w-44 truncate">{usedBy(file)}</span>
                ) : (
                  <span className="text-fg-4">{t("hf.unused")}</span>
                ),
            },
            { key: "menu", header: "", cell: menus },
          ]}
        />
      )}
      {action.isError && <FailureState error={action.error} size="sm" />}
    </div>
  );
}
function OllamaDownloaded({ account }: { account: ProviderAccountView }) {
  const { t, i18n } = useTranslation("settings");
  const api = useSettingsApi();
  const size = useByteSize();
  const action = useSettingsAction();
  const { phone } = useSettingsLayout();
  const [search, setSearch] = useState("");
  const models = useQuery({
    queryKey: [...browserKey, "ollama", account.id],
    queryFn: () => api.call("ollamaModelsList", { provider_account_id: account.id }),
  });
  const rows = (models.data?.models ?? []).filter((item) =>
    `${item.name} ${item.family ?? ""} ${item.quantization_level ?? ""}`
      .toLowerCase()
      .includes(search.trim().toLowerCase()),
  );
  const remove = (name: string) =>
    action.mutate(async () => {
      if (
        !(await confirm({
          title: t("hf.deleteServer", { name }),
          confirmLabel: t("hf.delete"),
          cancelLabel: t("manage.cancel"),
          destructive: true,
        }))
      )
        return;
      await api.call("ollamaModelDelete", {
        provider_account_id: account.id,
        model: name,
      });
      await models.refetch();
    });
  const deleteButton = (name: string) => (
    <IconButton
      size="sm"
      label={`${t("hf.delete")} ${name}`}
      disabled={action.isPending}
      onClick={() => remove(name)}
    >
      <Trash2 size={icon.sm} />
    </IconButton>
  );
  return (
    <div className="space-y-3">
      <div className="flex items-center gap-3">
        <div className="min-w-0 flex-1">
          <SearchField
            size="sm"
            value={search}
            onChange={setSearch}
            placeholder={t("hf.serverSearch")}
            aria-label={t("hf.serverSearch")}
          />
        </div>
        {models.data && (
          <Text variant="hint" className="shrink-0 tabular-nums">
            {t("hf.modelsTotal", {
              count: models.data.models.length,
              size: size(models.data.models.reduce((sum, item) => sum + (item.size ?? 0), 0)),
            })}
          </Text>
        )}
      </div>
      {models.isPending ? (
        <LoadingState />
      ) : models.isError ? (
        <FailureState error={models.error} onRetry={() => void models.refetch()} />
      ) : rows.length === 0 ? (
        <EmptyState
          title={t("hf.serverEmpty", { name: account.label })}
          description={t("hf.serverEmptyBody")}
        />
      ) : phone ? (
        <ListGroup>
          {rows.map((item) => (
            <ListRow
              key={item.name}
              title={<span className="block truncate font-mono text-sm">{item.name}</span>}
              description={[
                item.family,
                item.parameter_size,
                item.quantization_level,
                item.size === null ? null : size(item.size),
              ]
                .filter(Boolean)
                .join(" · ")}
              trailing={deleteButton(item.name)}
            />
          ))}
        </ListGroup>
      ) : (
        <Table
          rows={rows}
          rowKey={(item) => item.name}
          columns={[
            {
              key: "name",
              header: t("hf.name"),
              cell: (item) => (
                <span className="block max-w-96 truncate font-mono text-sm">{item.name}</span>
              ),
            },
            {
              key: "family",
              header: t("hf.family"),
              cell: (item) => (
                <span className="font-mono text-xs text-fg-2">{item.family ?? t("hf.unknown")}</span>
              ),
            },
            {
              key: "params",
              header: t("hf.params"),
              cell: (item) => item.parameter_size ?? t("hf.unknown"),
            },
            {
              key: "quant",
              header: t("hf.quant"),
              cell: (item) => (
                <span className="font-mono text-xs text-fg-2">
                  {item.quantization_level ?? t("hf.unknown")}
                </span>
              ),
            },
            {
              key: "size",
              header: t("hf.size"),
              numeric: true,
              cell: (item) => (item.size === null ? t("hf.unknown") : size(item.size)),
            },
            {
              key: "changed",
              header: t("hf.changed"),
              cell: (item) =>
                item.modified_at
                  ? relativeTime(new Date(item.modified_at), undefined, i18n.language)
                  : t("hf.unknown"),
            },
            { key: "delete", header: "", cell: (item) => deleteButton(item.name) },
          ]}
        />
      )}
      {action.isError && <FailureState error={action.error} size="sm" />}
    </div>
  );
}
