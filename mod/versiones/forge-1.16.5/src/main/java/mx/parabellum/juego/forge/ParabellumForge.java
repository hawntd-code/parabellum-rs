package mx.parabellum.juego.forge;

import mx.parabellum.juego.Juego;
import mx.parabellum.juego.mc.Base;
import net.minecraftforge.client.event.RenderGameOverlayEvent;
import net.minecraftforge.common.MinecraftForge;
import net.minecraftforge.event.TickEvent;
import net.minecraftforge.eventbus.api.IEventBus;
import net.minecraftforge.fml.ExtensionPoint;
import net.minecraftforge.fml.ModLoadingContext;
import net.minecraftforge.fml.client.registry.ClientRegistry;
import net.minecraftforge.fml.common.Mod;
import net.minecraftforge.fml.event.lifecycle.FMLClientSetupEvent;
import net.minecraftforge.fml.javafmlmod.FMLJavaModLoadingContext;
import net.minecraftforge.fml.loading.FMLEnvironment;
import net.minecraftforge.fml.network.FMLNetworkConstants;
import org.apache.commons.lang3.tuple.Pair;
import net.minecraftforge.client.event.GuiScreenEvent;
import net.minecraft.client.gui.screen.IngameMenuScreen;

/** Forge 1.16.5: la tecla, el tick y los avisos van con eventos (Java 8). */
@Mod("parabellum")
public final class ParabellumForge extends Base {
    public ParabellumForge() {
        // Solo del lado del cliente: el servidor no tiene que tenerlo.
        ModLoadingContext.get().registerExtensionPoint(ExtensionPoint.DISPLAYTEST, () -> Pair.of(() -> FMLNetworkConstants.IGNORESERVERONLY, (version, servidor) -> true));
        if (!FMLEnvironment.dist.isClient()) return;
        IEventBus bus = FMLJavaModLoadingContext.get().getModEventBus();
        bus.addListener((FMLClientSetupEvent evento) -> {
            ClientRegistry.registerKeyBinding(ABRIR);
            Juego.iniciar(this);
        });
        MinecraftForge.EVENT_BUS.addListener((TickEvent.ClientTickEvent evento) -> {
            if (evento.phase == TickEvent.Phase.END) tick();
        });
        MinecraftForge.EVENT_BUS.addListener((RenderGameOverlayEvent.Post evento) -> {
            if (evento.getType() == RenderGameOverlayEvent.ElementType.ALL) dibujarAvisos(evento.getMatrixStack());
        });
        MinecraftForge.EVENT_BUS.addListener((GuiScreenEvent.InitGuiEvent.Post evento) -> {
            if (evento.getGui() instanceof IngameMenuScreen) evento.addWidget(botonPausa(evento.getGui()));
        });
    }

    @Override
    protected String loader() {
        return "Forge";
    }
}
