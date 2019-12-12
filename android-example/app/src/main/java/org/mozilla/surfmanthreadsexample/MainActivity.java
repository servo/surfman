package org.mozilla.surfmanthreadsexample;

import android.opengl.GLSurfaceView;
import android.support.design.widget.FloatingActionButton;
import android.support.v7.app.AppCompatActivity;
import android.os.Bundle;
import android.view.View;

public class MainActivity extends AppCompatActivity {

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        this.setContentView(R.layout.activity_main);

        final SurfmanThreadsExampleRenderer renderer = new SurfmanThreadsExampleRenderer(this);

        GLSurfaceView surfaceView = this.findViewById(R.id.surface_view);
        surfaceView.setEGLContextClientVersion(3);
        surfaceView.setRenderer(renderer);

        FloatingActionButton runTestsButton = this.findViewById(R.id.run_tests_button);
        runTestsButton.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                renderer.runTests();
            }
        });
    }
}
