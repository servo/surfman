package org.mozilla.surfmanthreadsexample;

import android.opengl.GLSurfaceView;
import android.support.v7.app.AppCompatActivity;
import android.os.Bundle;

public class MainActivity extends AppCompatActivity {

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        this.setContentView(R.layout.activity_main);

        GLSurfaceView surfaceView = this.findViewById(R.id.surface_view);
        surfaceView.setRenderer(new SurfmanThreadsExampleRenderer());
    }
}
