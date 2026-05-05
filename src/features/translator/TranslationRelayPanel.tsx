import { Card, CardContent } from '@/components/ui/card';

export function TranslationRelayPanel() {
  return (
    <div className="p-6 max-w-3xl">
      <Card>
        <CardContent className="pt-6 text-sm text-muted-foreground">
          Translation Relay 开发中，当前已从主路径撤回，待后端与 adapter 闭环后再接回。
        </CardContent>
      </Card>
    </div>
  );
}

export function TranslationRelayView() {
  return (
    <div className="p-6 max-w-3xl">
      <Card>
        <CardContent className="pt-6 text-sm text-muted-foreground">
          Translation Relay 开发中，当前已从 Web 主路径撤回。
        </CardContent>
      </Card>
    </div>
  );
}
